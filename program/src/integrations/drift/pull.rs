use pinocchio::{
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
    error::SvmAlmControllerErrors,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PullArgs,
    integrations::drift::{
        constants::DRIFT_PROGRAM_ID,
        cpi::Withdraw,
        pdas::{
            derive_drift_spot_market_vault_pda, derive_drift_state_pda, derive_drift_user_stats_pda,
        },
        shared_sync::sync_drift_balance,
        utils::find_spot_market_account_info_by_id,
    },
    processor::PullAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

define_account_struct! {
    pub struct PullDriftAccounts<'info> {
        state: @owner(DRIFT_PROGRAM_ID);
        user: mut @owner(DRIFT_PROGRAM_ID);
        user_stats: mut @owner(DRIFT_PROGRAM_ID);
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_signer;
        // this account is checked inside reserve.sync_balance
        reserve_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
    }
}

impl<'info> PullDriftAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        outer_ctx: &'info PullAccounts,
        spot_market_index: u16,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(outer_ctx.remaining_accounts)?;
        let config = match config {
            IntegrationConfig::Drift(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        config.check_accounts(
            outer_ctx.controller_authority.key(),
            ctx.user.key(),
            spot_market_index,
        )?;

        let drift_state_pda = derive_drift_state_pda()?;
        if drift_state_pda.ne(ctx.state.key()) {
            msg! {"drift state: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let drift_user_stats_pda =
            derive_drift_user_stats_pda(outer_ctx.controller_authority.key())?;
        if drift_user_stats_pda.ne(ctx.user_stats.key()) {
            msg! {"drift user stats: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let spot_market_vault_pda = derive_drift_spot_market_vault_pda(spot_market_index)?;
        if spot_market_vault_pda.ne(ctx.spot_market_vault.key()) {
            msg! {"drift spot market vault: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        Ok(ctx)
    }
}

/// This function performs a "Pull" on a `DriftIntegration`.
/// Invokes Drift Withdraw instruction
pub fn process_pull_drift(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PullAccounts,
    outer_args: &PullArgs,
) -> ProgramResult {
    msg!("process_pull_drift");

    let (market_index, amount) = match outer_args {
        PullArgs::Drift {
            market_index,
            amount,
        } => (*market_index, *amount),
        _ => return Err(ProgramError::InvalidArgument),
    };

    if amount == 0 {
        msg! {"amount: must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    if !permission.can_reallocate() && !permission.can_liquidate(integration) {
        msg! {"permission: can_reallocate or can_liquidate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx =
        PullDriftAccounts::checked_from_accounts(&integration.config, &outer_ctx, market_index)?;

    // Sync the reserve balance before doing anything else
    reserve.sync_balance(
        inner_ctx.reserve_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    let spot_market_info =
        find_spot_market_account_info_by_id(inner_ctx.remaining_accounts, market_index)?;

    sync_drift_balance(
        controller,
        integration,
        outer_ctx.integration.key(),
        outer_ctx.controller.key(),
        outer_ctx.controller_authority,
        spot_market_info,
        inner_ctx.user,
    )?;

    let reserve_balance_before = reserve.last_balance;

    let spot_market_vault = TokenAccount::from_account_info(inner_ctx.spot_market_vault)?;
    let spot_market_vault_balance_before = spot_market_vault.amount();
    drop(spot_market_vault);

    Withdraw {
        state: &inner_ctx.state,
        user: &inner_ctx.user,
        user_stats: &inner_ctx.user_stats,
        authority: &outer_ctx.controller_authority,
        spot_market_vault: &inner_ctx.spot_market_vault,
        drift_signer: &inner_ctx.drift_signer,
        user_token_account: &inner_ctx.reserve_vault,
        token_program: &inner_ctx.token_program,
        // NOTE: we do not support TransferHooks with non-null programs.
        // If we ever do, then the TransferHooks extra accounts
        // must be included in remaining_accounts.
        // https://github.com/drift-labs/protocol-v2/blob/c3a43e411def66c74d2bc0063bd8268e2037eb7b/programs/drift/src/instructions/user.rs#L987
        remaining_accounts: &inner_ctx.remaining_accounts,
        market_index: market_index,
        amount: amount,
        // Borrows are not supported by this integration, so we can
        // always set reduce_only to true to prevent the possibility
        // of over withdrawing and incurring debt.
        reduce_only: true,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    let reserve_vault = TokenAccount::from_account_info(inner_ctx.reserve_vault)?;
    let reserve_balance_after = reserve_vault.amount();
    let reserve_mint = reserve_vault.mint();
    let net_inflow = reserve_balance_after.saturating_sub(reserve_balance_before);

    let spot_market_vault = TokenAccount::from_account_info(inner_ctx.spot_market_vault)?;
    let spot_market_vault_balance_after = spot_market_vault.amount();
    let spot_market_vault_delta =
        spot_market_vault_balance_before.saturating_sub(spot_market_vault_balance_after);

    // Emit accounting event for debit integration
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: Some(*outer_ctx.integration.key()),
            mint: *reserve_mint,
            reserve: None,
            direction: AccountingDirection::Debit,
            action: AccountingAction::Withdrawal,
            delta: spot_market_vault_delta,
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
            mint: *reserve_mint,
            reserve: Some(*outer_ctx.reserve_a.key()),
            direction: AccountingDirection::Credit,
            action: AccountingAction::Withdrawal,
            delta: net_inflow,
        }),
    )?;

    let clock = Clock::get()?;
    // Update integration and reserve rate limits for inflow
    integration.update_rate_limit_for_inflow(clock, net_inflow)?;
    reserve.update_for_inflow(clock, net_inflow)?;

    Ok(())
}
