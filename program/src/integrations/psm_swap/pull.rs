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
    integrations::psm_swap::{
        cpi::RemoveLiquidityFromPsmToken, push_pull_accounts::PushPullPsmSwapAccounts,
        shared_sync::sync_psm_liquidity_supplied,
    },
    processor::PullAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

pub fn process_pull_psm_swap(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PullAccounts,
    outer_args: &PullArgs,
) -> Result<(), ProgramError> {
    msg!("process_pull_psm_swap");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PullArgs::PsmSwap { amount } => *amount,
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

    let inner_ctx = PushPullPsmSwapAccounts::checked_from_accounts(
        outer_ctx.controller_authority.key(),
        &integration.config,
        outer_ctx.remaining_accounts,
        reserve,
    )?;

    // sync reserve before CPI
    reserve.sync_balance(
        inner_ctx.reserve_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    let psm_vault_balance_before = sync_psm_liquidity_supplied(
        controller,
        integration,
        outer_ctx.controller.key(),
        outer_ctx.integration.key(),
        inner_ctx.mint.key(),
        outer_ctx.controller_authority,
        inner_ctx.psm_token_vault,
    )?;

    let reserve_vault_balance_before =
        TokenAccount::from_account_info(inner_ctx.reserve_vault)?.amount();

    // CPI into PSM to remove liquidity
    RemoveLiquidityFromPsmToken {
        liquidity_owner: outer_ctx.controller_authority,
        psm_pool: inner_ctx.psm_pool,
        psm_token: inner_ctx.psm_token,
        mint: inner_ctx.mint,
        token_vault: inner_ctx.psm_token_vault,
        owner_token_account: inner_ctx.reserve_vault,
        token_program: inner_ctx.token_program,
        associated_token_program: inner_ctx.associated_token_program,
        amount,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    let psm_vault_balance_after =
        TokenAccount::from_account_info(inner_ctx.psm_token_vault)?.amount();

    let reserve_vault_balance_after =
        TokenAccount::from_account_info(inner_ctx.reserve_vault)?.amount();

    let psm_vault_delta = psm_vault_balance_before
        .checked_sub(psm_vault_balance_after)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    let reserve_vault_delta = reserve_vault_balance_after
        .checked_sub(reserve_vault_balance_before)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    // Emit accounting event for debit Integration
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: Some(*outer_ctx.integration.key()),
            mint: *inner_ctx.mint.key(),
            reserve: None,
            direction: AccountingDirection::Debit,
            action: AccountingAction::Withdrawal,
            delta: psm_vault_delta,
        }),
    )?;

    // Emit accounting event for credit Reserve
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: None,
            mint: *inner_ctx.mint.key(),
            reserve: Some(*outer_ctx.reserve_a.key()),
            direction: AccountingDirection::Credit,
            action: AccountingAction::Withdrawal,
            delta: reserve_vault_delta,
        }),
    )?;

    // Update the state
    match &mut integration.state {
        IntegrationState::PsmSwap(state) => {
            state.liquidity_supplied = psm_vault_balance_after;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    // update the integration rate limit for inflow
    integration.update_rate_limit_for_inflow(clock, reserve_vault_delta)?;

    // update the reserves for the flows
    reserve.update_for_inflow(clock, reserve_vault_delta)?;

    Ok(())
}
