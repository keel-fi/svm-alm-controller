use pinocchio::{
    instruction::{Seed, Signer}, msg, program_error::ProgramError, sysvars::{clock::Clock, Sysvar}, ProgramResult
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED, define_account_struct, events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent}, instructions::PushArgs, integrations::drift::{
        constants::DRIFT_PROGRAM_ID, cpi::PushDrift, protocol_state::{User, UserStats}
    }, processor::PushAccounts, state::{Controller, Integration, Permission, Reserve}
};

define_account_struct! {
    pub struct PushDriftAccounts<'info> {
        state;
        user: mut;
        user_stats: mut;
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        user_token_account: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_vault;
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
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

    let inner_ctx = PushDriftAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    // Validate Drift CPI constraints before making the call

    // 1. Validate that the controller authority can sign for the user (can_sign_for_user constraint)
    let user_data = inner_ctx.user.try_borrow_data()?;
    let user = User::try_from(&user_data)?;
    if user.authority != *outer_ctx.controller_authority.key() {
        msg!("authority: controller authority cannot sign for user");
        return Err(ProgramError::IncorrectAuthority);
    }

    // 2. Validate that the user_stats belongs to the controller authority (is_stats_for_user constraint)
    let user_stats_data = inner_ctx.user_stats.try_borrow_data()?;
    let user_stats = UserStats::try_from(&user_stats_data)?;
    if user_stats.authority != *outer_ctx.controller_authority.key() {
        msg!("user_stats: does not belong to controller authority");
        return Err(ProgramError::InvalidAccountData);
    }

    if user_stats.authority != user.authority {
        msg!("user_stats authority does not match user authority");
        return Err(ProgramError::InvalidAccountData);
    }

    drop(user);
    drop(user_stats);
    drop(user_stats_data);
    drop(user_data);

    // 3. Verify that the spot_market_vault mint matches the user_token_account mint
    let spot_market_vault = TokenAccount::from_account_info(&inner_ctx.spot_market_vault)?;
    let user_token_account = TokenAccount::from_account_info(&inner_ctx.user_token_account)?;
    if spot_market_vault.mint() != user_token_account.mint() {
        msg!("mint mismatch: spot_market_vault mint != user_token_account mint");
        return Err(ProgramError::InvalidArgument);
    }

    drop(spot_market_vault);
    drop(user_token_account);

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

    PushDrift {
        state: inner_ctx.state,
        user: inner_ctx.user,
        user_stats: inner_ctx.user_stats,
        authority: outer_ctx.controller_authority,
        spot_market_vault: inner_ctx.spot_market_vault,
        user_token_account: inner_ctx.user_token_account,
        token_program: inner_ctx.token_program,
        remaining_accounts: &inner_ctx.remaining_accounts,
        market_index,
        amount,
        reduce_only,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    msg!("drift push successful");

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
