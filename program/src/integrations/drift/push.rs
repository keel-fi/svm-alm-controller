use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::drift::cpi::PushDrift,
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

define_account_struct! {
    pub struct PushDriftAccounts<'info> {
        state;
        user: mut;
        user_stats: mut;
        authority: signer;
        spot_market_vault: mut;
        user_token_account: mut;
        rent;
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        system_program;
    }
}

pub fn process_push_drift(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> ProgramResult {
    msg!("process_push_drift");

    let amount = match outer_args {
        PushArgs::Drift { amount } => *amount,
        _ => return Err(ProgramError::InvalidArgument),
    };

    if amount == 0 {
        msg! {"amount: must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    if !permission.can_reallocate() {
        msg! {"permission: can_reallocate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushDriftAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    reserve.sync_balance(
        inner_ctx.spot_market_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    let vault = TokenAccount::from_account_info(&inner_ctx.spot_market_vault)?;
    let liquidity_amount_before = vault.amount();

    PushDrift {
        user: inner_ctx.user,
        user_stats: inner_ctx.user_stats,
        authority: inner_ctx.authority,
        spot_market_vault: inner_ctx.spot_market_vault,
        user_token_account: inner_ctx.user_token_account,
        rent: inner_ctx.rent,
        system_program: inner_ctx.system_program,
        amount,
    }
    .invoke()?;

    let vault = TokenAccount::from_account_info(&inner_ctx.spot_market_vault)?;
    let liquidity_amount_after = vault.amount();
    let liquidity_amount_delta = liquidity_amount_after.saturating_sub(liquidity_amount_before);

    // TODO: calculate liquidity_value_delta
    let liquidity_value_delta = 100;

    if liquidity_amount_delta > 0 && liquidity_value_delta > 0 {
        // Emit accounting event for credit Integration
        controller.emit_event(
            outer_ctx.controller_authority,
            outer_ctx.controller.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: Some(*outer_ctx.integration.key()),
                mint: *inner_ctx.spot_market_vault.key(),
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
                mint: *inner_ctx.spot_market_vault.key(),
                reserve: Some(*outer_ctx.reserve_a.key()),
                direction: AccountingDirection::Debit,
                action: AccountingAction::Deposit,
                delta: liquidity_amount_delta
            }),
        )?;
    }

    Ok(())
}
