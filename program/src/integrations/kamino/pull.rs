use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    enums::IntegrationState,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PullArgs,
    integrations::kamino::{
        balance::get_kamino_lending_balance,
        cpi::WithdrawObligationCollateralAndRedeemReserveCollateralV2,
        klend_protocol_state::KaminoReserve, push_pull_accounts::PushPullKaminoAccounts,
        shared_sync::sync_kamino_liquidity_value,
    },
    processor::PullAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

/// This function performs a "Pull" on a `KaminoIntegration`.
/// In order to do so it:
/// - CPIs into KLEND program.
/// - Tracks the change in balances, similar to how `process_push_kamino` works.
pub fn process_pull_kamino(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PullAccounts,
    outer_args: &PullArgs,
) -> Result<(), ProgramError> {
    msg!("process_pull_kamino");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PullArgs::Kamino { amount } => *amount,
        _ => return Err(ProgramError::InvalidArgument),
    };

    if amount == 0 {
        msg! {"amount must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    if !permission.can_reallocate() && !permission.can_liquidate(&integration) {
        msg! {"permission: can_reallocate or can_liquidate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushPullKaminoAccounts::checked_from_accounts(
        outer_ctx.controller_authority.key(),
        &integration.config,
        outer_ctx.remaining_accounts,
        reserve,
    )?;

    if inner_ctx
        .obligation
        .owner()
        .ne(inner_ctx.kamino_program.key())
    {
        msg! {"obligation: invalid owner"};
        return Err(ProgramError::IllegalOwner);
    }

    reserve.sync_balance(
        inner_ctx.reserve_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // Accounting event for changes in liquidity value BEFORE withdraw
    sync_kamino_liquidity_value(
        controller,
        integration,
        outer_ctx.integration.key(),
        outer_ctx.controller.key(),
        outer_ctx.controller_authority,
        inner_ctx.kamino_reserve_liquidity_mint.key(),
        inner_ctx.kamino_reserve,
        inner_ctx.obligation,
    )?;

    // Kamino Withdraw uses Collateral (aka shares) instead of
    // the liquidity tokens directly. To maintain the same mechanics
    // across other integrations, we convert from the liquidity amount
    // to the collateral amount here.
    let kamino_reserve_data = inner_ctx.kamino_reserve.try_borrow_data()?;
    let kamino_reserve_state = KaminoReserve::try_from_slice(&kamino_reserve_data)?;
    let collateral_amount = kamino_reserve_state.liquidity_to_collateral(amount);
    drop(kamino_reserve_data);

    let liquidity_amount_before = {
        let vault = TokenAccount::from_account_info(inner_ctx.reserve_vault)?;
        vault.amount()
    };

    let liquidity_value_before =
        get_kamino_lending_balance(inner_ctx.kamino_reserve, inner_ctx.obligation)?;

    WithdrawObligationCollateralAndRedeemReserveCollateralV2 {
        owner: outer_ctx.controller_authority,
        obligation: inner_ctx.obligation,
        lending_market: inner_ctx.market,
        market_authority: inner_ctx.market_authority,
        kamino_reserve: inner_ctx.kamino_reserve,
        reserve_liquidity_mint: inner_ctx.kamino_reserve_liquidity_mint,
        reserve_collateral_supply: inner_ctx.kamino_reserve_collateral_supply,
        reserve_collateral_mint: inner_ctx.kamino_reserve_collateral_mint,
        reserve_liquidity_supply: inner_ctx.kamino_reserve_liquidity_supply,
        user_liquidity_destination: inner_ctx.reserve_vault,
        // placeholder AccountInfo
        placeholder_user_destination_collateral: inner_ctx.kamino_program,
        collateral_token_program: inner_ctx.collateral_token_program,
        liquidity_token_program: inner_ctx.liquidity_token_program,
        instruction_sysvar: inner_ctx.instruction_sysvar_account,
        obligation_farm_user_state: inner_ctx.obligation_farm_collateral,
        reserve_farm_state: inner_ctx.reserve_farm_collateral,
        farms_program: inner_ctx.kamino_farms_program,
        collateral_amount,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    // For liquidity and collateral amount calculation
    let liquidity_amount_after = {
        let vault = TokenAccount::from_account_info(inner_ctx.reserve_vault)?;
        vault.amount()
    };
    let liquidity_amount_delta = liquidity_amount_after.saturating_sub(liquidity_amount_before);

    let liquidity_value_after =
        get_kamino_lending_balance(inner_ctx.kamino_reserve, inner_ctx.obligation)?;
    let liquidity_value_delta = liquidity_value_before.saturating_sub(liquidity_value_after);

    // Emit accounting event for debit integration
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: Some(*outer_ctx.integration.key()),
            mint: *inner_ctx.kamino_reserve_liquidity_mint.key(),
            reserve: None,
            direction: AccountingDirection::Debit,
            action: AccountingAction::Withdrawal,
            delta: liquidity_value_delta,
        }),
    )?;

    // Emit accounting event for credit Reserve
    // Note: this is to ensure there is double accounting
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: None,
            mint: *inner_ctx.kamino_reserve_liquidity_mint.key(),
            reserve: Some(*outer_ctx.reserve_a.key()),
            direction: AccountingDirection::Credit,
            action: AccountingAction::Withdrawal,
            delta: liquidity_amount_delta,
        }),
    )?;

    // Update the state
    match &mut integration.state {
        IntegrationState::Kamino(state) => {
            state.balance = liquidity_value_after;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    // Update the integration rate limit for inflow
    integration.update_rate_limit_for_inflow(clock, liquidity_amount_delta)?;

    // Update the reserves for the flows
    reserve.update_for_inflow(clock, liquidity_amount_delta)?;

    Ok(())
}
