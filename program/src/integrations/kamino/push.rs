use pinocchio::{
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_token::state::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    enums::IntegrationState,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::kamino::{
        cpi::DepositReserveLiquidityV2,
        protocol_state::{get_liquidity_and_lp_amount, Obligation},
        shared_sync::sync_kamino_liquidity_value,
        validations::PushPullKaminoAccounts,
    },
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

/// This function performs a "Push" on a `KaminoIntegration`.
/// In order to do so it:
/// - CPIs into KLEND program.
/// - Tracks the change in balance of `liquidity_source` account (our vault)
/// and `liquidity_value` from Kamino state
/// - Updates the `liquidity_value` and `lp_token_amount` of the integration by reading
///     the `obligation` and the `kamino_reserve` after the CPI.
pub fn process_push_kamino(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    msg!("process_push_kamino");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PushArgs::Kamino { amount } => *amount,
        _ => return Err(ProgramError::InvalidArgument),
    };

    if amount == 0 {
        msg! {"amount must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    if !permission.can_reallocate() {
        msg! {"permission: can_reallocate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushPullKaminoAccounts::checked_from_accounts(
        outer_ctx.controller_authority.key(),
        &integration.config,
        outer_ctx.remaining_accounts,
        reserve,
    )?;

    // Push fails if the obligation is full and the current obligation collateral
    // is not included in one of the 8 slots.
    // Use of an inner scrop to avoid borrowing issues
    {
        let obligation_data = inner_ctx.obligation.try_borrow_data()?;
        let obligation = Obligation::load_checked(&obligation_data)?;
        if obligation.is_deposits_full()
            && obligation
                .get_obligation_collateral_for_reserve(inner_ctx.kamino_reserve.key())
                .is_none()
        {
            msg! {"Obligation: invalid obligation, collateral deposit slots are full"}
            return Err(ProgramError::InvalidAccountData);
        }
    }

    reserve.sync_balance(
        inner_ctx.token_account,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // Accounting event for changes in liquidity value BEFORE deposit
    sync_kamino_liquidity_value(
        controller,
        integration,
        outer_ctx.integration.key(),
        outer_ctx.controller.key(),
        outer_ctx.controller_authority,
        inner_ctx.reserve_liquidity_mint.key(),
        inner_ctx.kamino_reserve,
        inner_ctx.obligation,
    )?;

    // This is for calculating the exact amount leaving our vault during reposit
    let liquidity_amount_before = {
        let vault = TokenAccount::from_account_info(inner_ctx.token_account)?;
        vault.amount()
    };

    let (liquidity_value_before, _) =
        get_liquidity_and_lp_amount(inner_ctx.kamino_reserve, inner_ctx.obligation)?;

    // Perform kamino deposit liquidity cpi
    DepositReserveLiquidityV2 {
        owner: outer_ctx.controller_authority,
        obligation: inner_ctx.obligation,
        lending_market: inner_ctx.market,
        market_authority: inner_ctx.market_authority,
        kamino_reserve: inner_ctx.kamino_reserve,
        reserve_liquidity_mint: inner_ctx.reserve_liquidity_mint,
        reserve_liquidity_supply: inner_ctx.reserve_liquidity_supply,
        reserve_collateral_mint: inner_ctx.reserve_collateral_mint,
        reserve_collateral_supply: inner_ctx.reserve_collateral_supply,
        user_source_liquidity: inner_ctx.token_account,
        // placeholder AccountInfo
        placeholder_user_destination_collateral: inner_ctx.kamino_program,
        collateral_token_program: inner_ctx.collateral_token_program,
        liquidity_token_program: inner_ctx.liquidity_token_program,
        instruction_sysvar: inner_ctx.instruction_sysvar_account,
        obligation_farm_user_state: inner_ctx.obligation_farm_collateral,
        reserve_farm_state: inner_ctx.reserve_farm_collateral,
        farms_program: inner_ctx.kamino_farms_program,
        liquidity_amount: amount,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    // This is for calculating the exact amount leaving our vault during reposit
    let liquidity_amount_after = {
        let vault = TokenAccount::from_account_info(inner_ctx.token_account)?;
        vault.amount()
    };
    let liquidity_amount_delta = liquidity_amount_before.saturating_sub(liquidity_amount_after);

    let (liquidity_value_after, lp_amount_after) =
        get_liquidity_and_lp_amount(inner_ctx.kamino_reserve, inner_ctx.obligation)?;
    let liquidity_value_delta = liquidity_value_after.saturating_sub(liquidity_value_before);

    // In order to reflect the actual value of the liquidity deposit,
    // we use kamino's calculations (liquidity value delta)

    // Emit accounting event for credit Integration
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: Some(*outer_ctx.integration.key()),
            mint: *inner_ctx.reserve_liquidity_mint.key(),
            reserve: None,
            direction: AccountingDirection::Credit,
            action: AccountingAction::Deposit,
            delta: liquidity_value_delta,
        }),
    )?;

    // Emit accounting event for debit Reserve
    // Note: this is to ensure there is double accounting
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: None,
            mint: *inner_ctx.reserve_liquidity_mint.key(),
            reserve: Some(*outer_ctx.reserve_a.key()),
            direction: AccountingDirection::Debit,
            action: AccountingAction::Deposit,
            delta: liquidity_amount_delta,
        }),
    )?;

    // Update the state
    match &mut integration.state {
        IntegrationState::Kamino(kamino_state) => {
            kamino_state.last_liquidity_value = liquidity_value_after;
            kamino_state.last_lp_amount = lp_amount_after;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    // update the integration rate limit for outflow
    integration.update_rate_limit_for_outflow(clock, liquidity_amount_delta)?;

    // update the reserves for the flows
    reserve.update_for_outflow(clock, liquidity_amount_delta, false)?;

    Ok(())
}
