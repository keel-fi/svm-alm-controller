use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, 
    msg, 
    program_error::ProgramError, 
    sysvars::{clock::Clock, Sysvar}
};
use pinocchio_token::state::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED, 
    enums::IntegrationState, 
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent}, 
    instructions::PullArgs, 
    integrations::kamino::{
        cpi::withdraw_obligation_collateral_v2_cpi, 
        kamino_state::get_liquidity_and_lp_amount, 
        shared_sync::sync_kamino_liquidity_value, 
        validations::PushPullKaminoAccounts
    }, 
    processor::PullAccounts, 
    state::{Controller, Integration, Permission, Reserve}
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
    outer_args: &PullArgs
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

    if !permission.can_reallocate() {
        msg! {"permission: can_reallocate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushPullKaminoAccounts::checked_from_accounts(
        outer_ctx.controller_authority.key(),
        &integration.config, 
        outer_ctx.remaining_accounts,
        reserve
    )?;

    reserve.sync_balance(
        inner_ctx.token_account, 
        outer_ctx.controller_authority, 
        outer_ctx.controller.key(), 
        controller
    )?;

    // accounting event for changes in liquidity value BEFORE withdraw
    sync_kamino_liquidity_value(
        controller, 
        integration, 
        outer_ctx.integration.key(), 
        outer_ctx.controller.key(), 
        outer_ctx.controller_authority, 
        inner_ctx.reserve_liquidity_mint.key(), 
        inner_ctx.kamino_reserve, 
        inner_ctx.obligation
    )?;

    let liquidity_amount_before = {
        let vault
            = TokenAccount::from_account_info(inner_ctx.token_account)?;
        vault.amount()
    };

    let (liquidity_value_before, _) = get_liquidity_and_lp_amount(
        inner_ctx.kamino_reserve, 
        inner_ctx.obligation
    )?;

    withdraw_obligation_collateral_v2(
        amount, 
        Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ]), 
        outer_ctx.controller_authority, 
        &inner_ctx
    )?;

    // for liquidity and collateral amount calculation
    let liquidity_amount_after = {
        let vault
            = TokenAccount::from_account_info(inner_ctx.token_account)?;
        vault.amount()
    };
    let liquidity_amount_delta = liquidity_amount_after.saturating_sub(liquidity_amount_before);

    let (liquidity_value_after, lp_amount_after) = get_liquidity_and_lp_amount(
        inner_ctx.kamino_reserve, 
        inner_ctx.obligation
    )?;
    let liquidity_value_delta = liquidity_value_before.saturating_sub(liquidity_value_after);
    
    // Emit accounting event for debit integration
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: Some(*outer_ctx.integration.key()),
            mint: *inner_ctx.reserve_liquidity_mint.key(),
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
            mint: *inner_ctx.reserve_liquidity_mint.key(),
            reserve: Some(*outer_ctx.reserve_a.key()),
            direction: AccountingDirection::Credit,
            action: AccountingAction::Withdrawal,
            delta: liquidity_amount_delta,
        }),
    )?;
    
    // update the state
    match &mut integration.state {
        IntegrationState::Kamino(kamino_state) => {
            kamino_state.last_liquidity_value = liquidity_value_after;
            kamino_state.last_lp_amount = lp_amount_after;
        },
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }
    
    // update the integration rate limit for inflow
    integration.update_rate_limit_for_inflow(clock, liquidity_amount_delta)?;

    // update the reserves for the flows
    reserve.update_for_inflow(clock, liquidity_amount_delta)?;
    
    Ok(())
}

fn withdraw_obligation_collateral_v2(
    amount: u64,
    signer: Signer,
    owner: &AccountInfo,
    inner_ctx: &PushPullKaminoAccounts
) -> Result<(), ProgramError> {
    withdraw_obligation_collateral_v2_cpi(
        amount, 
        signer, 
        owner, 
        inner_ctx.obligation, 
        inner_ctx.market, 
        inner_ctx.market_authority, 
        inner_ctx.kamino_reserve, 
        inner_ctx.reserve_liquidity_mint, 
        inner_ctx.reserve_liquidity_supply, 
        inner_ctx.reserve_collateral_mint, 
        inner_ctx.reserve_collateral_supply, 
        inner_ctx.token_account, 
        inner_ctx.collateral_token_program, 
        inner_ctx.liquidity_token_program, 
        inner_ctx.instruction_sysvar_account, 
        inner_ctx.obligation_farm_collateral, 
        inner_ctx.reserve_farm_collateral, 
        inner_ctx.kamino_farms_program, 
        inner_ctx.kamino_program
    )?;

    Ok(())
}