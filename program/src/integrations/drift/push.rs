use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::IntegrationConfig,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::drift::{constants::DRIFT_PROGRAM_ID, cpi::Deposit},
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

define_account_struct! {
    pub struct PushDriftAccounts<'info> {
        state: @owner(DRIFT_PROGRAM_ID);
        user: mut @owner(DRIFT_PROGRAM_ID);
        user_stats: mut @owner(DRIFT_PROGRAM_ID);
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        user_token_account: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_vault: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
    }
}

impl<'info> PushDriftAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        accounts_infos: &'info [AccountInfo],
        spot_market_index: u16,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;
        let config = match config {
            IntegrationConfig::Drift(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if spot_market_index != config.spot_market_index {
            msg!("spot_market_index: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(ctx)
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

    let (market_index, amount, reduce_only) = match outer_args {
        PushArgs::Drift {
            market_index,
            amount,
            reduce_only,
        } => (*market_index, *amount, *reduce_only),
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

    let inner_ctx = PushDriftAccounts::checked_from_accounts(
        &integration.config,
        &outer_ctx.remaining_accounts,
        market_index,
    )?;

    // Sync the reserve balance before doing anything else
    reserve.sync_balance(
        inner_ctx.reserve_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // Track the user token account balance before the transfer
    let user_token_account = TokenAccount::from_account_info(&inner_ctx.user_token_account)?;
    let user_token_balance_before = user_token_account.amount();
    drop(user_token_account);

    let liquidity_value_account = TokenAccount::from_account_info(&inner_ctx.spot_market_vault)?;
    let liquidity_value_balance_before = liquidity_value_account.amount();
    drop(liquidity_value_account);

    Deposit {
        state: &inner_ctx.state,
        user: &inner_ctx.user,
        user_stats: &inner_ctx.user_stats,
        authority: &outer_ctx.controller_authority,
        spot_market_vault: &inner_ctx.spot_market_vault,
        user_token_account: &inner_ctx.user_token_account,
        token_program: &inner_ctx.token_program,
        remaining_accounts: &inner_ctx.remaining_accounts,
        market_index: market_index,
        amount: amount,
        reduce_only: reduce_only,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    // Reload the user token account to check its balance
    let user_token_account = TokenAccount::from_account_info(&inner_ctx.user_token_account)?;
    let user_token_balance_after = user_token_account.amount();
    let check_delta = user_token_balance_before
        .checked_sub(user_token_balance_after)
        .unwrap();
    if check_delta != amount {
        msg! {"check_delta: transfer did not match the user token account balance change"};
        return Err(ProgramError::InvalidArgument);
    }

    let liquidity_value_account = TokenAccount::from_account_info(&inner_ctx.spot_market_vault)?;
    let liquidity_value_balance_after = liquidity_value_account.amount();
    let liquidity_value_delta = liquidity_value_balance_after
        .checked_sub(liquidity_value_balance_before)
        .unwrap();

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
            delta: check_delta,
        }),
    )?;

    let clock = Clock::get()?;

    integration.update_rate_limit_for_outflow(clock, check_delta)?;
    reserve.update_for_outflow(clock, check_delta, false)?;

    Ok(())
}
