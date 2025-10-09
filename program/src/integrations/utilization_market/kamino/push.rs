use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, msg, 
    program_error::ProgramError, 
    sysvars::{clock::Clock, Sysvar}
};
use pinocchio_token::state::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED, 
    enums::IntegrationState, 
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent}, 
    instructions::PushArgs, 
    integrations::utilization_market::{
        kamino::{
            cpi::deposit_reserve_liquidity_v2_cpi, 
            kamino_state::{get_liquidity_and_lp_amount, Obligation}, 
            shared_sync::sync_kamino_liquidity_value, 
            validations::PushPullKaminoAccounts
        }, 
        state::UtilizationMarketState, 
    }, 
    processor::PushAccounts, 
    state::{Controller, Integration, Permission, Reserve}
};

/// This function performs a "Push" on a `KaminoIntegration`.
/// In order to do so it:
/// - CPIs into KLEND program.
/// - Tracks the change in balance of `liquidity_source` account (our vault) 
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
        reserve
    )?;

    // Push fails if the obligation is full and the current obligation collateral
    // is not included in one of the 8 slots.
    let obligation = Obligation::try_from(
        inner_ctx.obligation.try_borrow_data()?.as_ref()
    )?;
    if obligation.is_deposits_full() 
        && obligation.get_obligation_collateral_for_reserve(inner_ctx.kamino_reserve.key()).is_none() 
    {
        msg! {"Obligation: invalid obligation, collateral deposit slots are full"}
        return Err(ProgramError::InvalidAccountData)
    }
    drop(obligation);

    reserve.sync_balance(
        inner_ctx.token_account,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // accounting event for changes in liquidity value BEFORE deposit
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
    

    // This is for calculating the exact amount leaving our vault during reposit
    let liquidity_amount_before = {
        let vault 
            = TokenAccount::from_account_info(inner_ctx.token_account)?;
        vault.amount()
    };

    // perform kamino deposit liquidity cpi
    deposit_reserve_liquidity_v2(
        amount, 
        Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ]), 
        outer_ctx.controller_authority, 
        &inner_ctx
    )?;

    // This is for calculating the exact amount leaving our vault during reposit
    let liquidity_amount_after = {
        let vault 
            = TokenAccount::from_account_info(inner_ctx.token_account)?;
        vault.amount()
    };

    let liquidity_amount_delta = liquidity_amount_before.saturating_sub(liquidity_amount_after);

    if liquidity_amount_delta > 0 {
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
                delta: liquidity_amount_delta
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
                delta: liquidity_amount_delta
            }),
        )?;
    }

    let (liquidity_value_after_deposit, lp_amount_after_deposit) = get_liquidity_and_lp_amount(
        inner_ctx.kamino_reserve, 
        inner_ctx.obligation
    )?;

    // update the state
    match &mut integration.state {
        IntegrationState::UtilizationMarket(state) => {
            match state {
                UtilizationMarketState::KaminoState(kamino_state) => {
                    kamino_state.last_liquidity_value = liquidity_value_after_deposit;
                    kamino_state.last_lp_amount = lp_amount_after_deposit;
                }
            }
        },                   
        _ => return Err(ProgramError::InvalidAccountData),
    }

    // update the integration rate limit for outflow
    integration.update_rate_limit_for_outflow(clock, liquidity_amount_delta)?;

    // update the reserves for the flows
    // todo: verify allow_underflow
    reserve.update_for_outflow(clock, liquidity_amount_delta, false)?;

    Ok(())
}


fn deposit_reserve_liquidity_v2(
    amount: u64,
    signer: Signer,
    owner: &AccountInfo,
    inner_ctx: &PushPullKaminoAccounts
) -> Result<(), ProgramError> {
    deposit_reserve_liquidity_v2_cpi(
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
        inner_ctx.kamino_program, 
    )?;

    Ok(())
}