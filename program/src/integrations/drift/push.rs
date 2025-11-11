use account_zerocopy_deserialize::AccountZerocopyDeserialize;
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
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::drift::{
        balance::get_drift_lending_balance,
        constants::DRIFT_PROGRAM_ID,
        cpi::{Deposit, UpdateSpotMarketCumulativeInterest},
        pdas::{
            derive_drift_spot_market_vault_pda, derive_drift_state_pda, derive_drift_user_stats_pda,
        },
        protocol_state::SpotMarket,
        shared_sync::sync_drift_balance,
        utils::find_spot_market_and_oracle_account_info_by_id,
    },
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

define_account_struct! {
    pub struct PushDriftAccounts<'info> {
        state: @owner(DRIFT_PROGRAM_ID);
        user: mut @owner(DRIFT_PROGRAM_ID);
        user_stats: mut @owner(DRIFT_PROGRAM_ID);
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        // this account is checked inside reserve.sync_balance
        reserve_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
    }
}

impl<'info> PushDriftAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        outer_ctx: &'info PushAccounts,
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

pub fn process_push_drift(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> ProgramResult {
    msg!("process_push_drift");

    let (spot_market_index, amount) = match outer_args {
        PushArgs::Drift {
            spot_market_index,
            amount,
        } => (*spot_market_index, *amount),
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
        &outer_ctx,
        spot_market_index,
    )?;

    // Sync the reserve balance before doing anything else
    reserve.sync_balance(
        inner_ctx.reserve_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    let (spot_market_info, oracle_info) = find_spot_market_and_oracle_account_info_by_id(
        &inner_ctx.remaining_accounts,
        spot_market_index,
    )?;

    // Update Drift SpotMarket interest
    UpdateSpotMarketCumulativeInterest {
        state: inner_ctx.state,
        spot_market: spot_market_info,
        oracle: oracle_info,
        spot_market_vault: inner_ctx.spot_market_vault,
    }
    .invoke()?;

    sync_drift_balance(
        controller,
        integration,
        outer_ctx.integration.key(),
        outer_ctx.controller.key(),
        outer_ctx.controller_authority,
        spot_market_info,
        inner_ctx.user,
    )?;

    // Track the user token account balance before the transfer
    let reserve_vault = TokenAccount::from_account_info(&inner_ctx.reserve_vault)?;
    let reserve_vault_balance_before = reserve_vault.amount();
    drop(reserve_vault);

    let spot_market_vault = TokenAccount::from_account_info(&inner_ctx.spot_market_vault)?;
    let spot_market_vault_balance_before = spot_market_vault.amount();
    drop(spot_market_vault);

    Deposit {
        state: &inner_ctx.state,
        user: &inner_ctx.user,
        user_stats: &inner_ctx.user_stats,
        authority: &outer_ctx.controller_authority,
        spot_market_vault: &inner_ctx.spot_market_vault,
        user_token_account: &inner_ctx.reserve_vault,
        token_program: &inner_ctx.token_program,
        // NOTE: we do not support TransferHooks with non-null programs.
        // If we ever do, then the TransferHooks extra accounts
        // must be included in remaining_accounts.
        // https://github.com/drift-labs/protocol-v2/blob/c3a43e411def66c74d2bc0063bd8268e2037eb7b/programs/drift/src/instructions/user.rs#L789
        remaining_accounts: &inner_ctx.remaining_accounts,
        market_index: spot_market_index,
        amount: amount,
        // Borrows are not supported by this integration, so we can
        // always set reduce_only to false as we'll never be depositing
        // to pay down a borrow.
        reduce_only: false,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    // Reload the user token account to check its balance
    let reserve_vault = TokenAccount::from_account_info(&inner_ctx.reserve_vault)?;
    let reserve_mint = reserve_vault.mint();
    let reserve_vault_balance_after = reserve_vault.amount();
    let reserve_vault_balance_delta = reserve_vault_balance_before
        .checked_sub(reserve_vault_balance_after)
        .unwrap();
    if reserve_vault_balance_delta != amount {
        msg! {"reserve_vault_delta: transfer did not match the expected amount"};
        return Err(ProgramError::InvalidArgument);
    }

    let spot_market_vault = TokenAccount::from_account_info(&inner_ctx.spot_market_vault)?;
    let spot_market_vault_balance_after = spot_market_vault.amount();
    let liquidity_value_delta = spot_market_vault_balance_after
        .checked_sub(spot_market_vault_balance_before)
        .unwrap();

    // Emit accounting event for credit Integration
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: Some(*outer_ctx.integration.key()),
            mint: *reserve_mint,
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
            mint: *reserve_mint,
            reserve: Some(*outer_ctx.reserve_a.key()),
            direction: AccountingDirection::Debit,
            action: AccountingAction::Deposit,
            delta: reserve_vault_balance_delta,
        }),
    )?;

    // Update the state
    match &mut integration.state {
        IntegrationState::Drift(state) => {
            // Update amount to the Drift balance
            let spot_market_data = spot_market_info.try_borrow_data()?;
            let spot_market_state = SpotMarket::try_from_slice(&spot_market_data)?;
            state.balance = get_drift_lending_balance(spot_market_state, inner_ctx.user)?;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    let clock = Clock::get()?;

    // Update integration and reserve rate limits for inflow
    integration.update_rate_limit_for_outflow(clock, reserve_vault_balance_delta)?;
    reserve.update_for_outflow(clock, reserve_vault_balance_delta, false)?;

    Ok(())
}
