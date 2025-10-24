/* Drift LendingProcessor implementation */

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    integrations::{
        drift::{
            constants::DRIFT_PROGRAM_ID,
            cpi::{Deposit, Withdraw},
            shared_sync::sync_drift_balance,
            utils::find_spot_market_account_info_by_id,
        },
        shared::lending_processor::{LendingContext, LendingOperationResult, LendingProcessor},
    },
    state::Integration,
};

define_account_struct! {
    pub struct DriftAccounts<'info> {
        state: @owner(DRIFT_PROGRAM_ID);
        user: mut @owner(DRIFT_PROGRAM_ID);
        user_stats: mut @owner(DRIFT_PROGRAM_ID);
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_signer: @owner(DRIFT_PROGRAM_ID);
        reserve_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
    }
}

impl<'info> DriftAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        remaining_accounts: &'info [AccountInfo],
        market_index: u16,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(remaining_accounts)?;
        let config = match config {
            IntegrationConfig::Drift(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        config.check_accounts(
            &pinocchio::pubkey::Pubkey::default(), // This would need to be passed from caller
            ctx.user.key(),
            market_index,
        )?;

        Ok(ctx)
    }
}

pub struct DriftLendingProcessor {
    pub market_index: u16,
}

impl LendingProcessor for DriftLendingProcessor {
    fn get_current_balance(&self, ctx: &LendingContext) -> Result<u64, ProgramError> {
        match &ctx.integration.state {
            IntegrationState::Drift(state) => Ok(state.balance),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    fn update_integration_state(&self, integration: &mut Integration, new_balance: u64) -> Result<(), ProgramError> {
        match &mut integration.state {
            IntegrationState::Drift(state) => {
                state.balance = new_balance;
                Ok(())
            }
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    fn execute_deposit(&self, ctx: &mut LendingContext, amount: u64) -> Result<LendingOperationResult, ProgramError> {
        use pinocchio::instruction::{Seed, Signer};

        // Parse accounts
        let accounts = DriftAccounts::checked_from_accounts(
            &ctx.integration.config,
            &[], // This would need to be passed from the caller
            self.market_index,
        )?;

        // Track balances before operation
        let reserve_vault_before = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };

        let spot_market_vault_before = {
            let vault = TokenAccount::from_account_info(accounts.spot_market_vault)?;
            vault.amount()
        };

        // Execute deposit
        Deposit {
            state: &accounts.state,
            user: &accounts.user,
            user_stats: &accounts.user_stats,
            authority: ctx.controller_authority,
            spot_market_vault: &accounts.spot_market_vault,
            user_token_account: &accounts.reserve_vault,
            token_program: &accounts.token_program,
            remaining_accounts: &accounts.remaining_accounts,
            market_index: self.market_index,
            amount,
            reduce_only: false,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(ctx.controller_pubkey),
            Seed::from(&[ctx.controller.authority_bump]),
        ])])?;

        // Calculate deltas
        let reserve_vault_after = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };
        let reserve_delta = reserve_vault_before.saturating_sub(reserve_vault_after);

        let spot_market_vault_after = {
            let vault = TokenAccount::from_account_info(accounts.spot_market_vault)?;
            vault.amount()
        };
        let integration_delta = spot_market_vault_after.saturating_sub(spot_market_vault_before);

        // Get new balance
        let new_balance = self.get_current_balance(ctx)? + integration_delta;

        Ok(LendingOperationResult {
            integration_delta,
            reserve_delta,
            new_balance,
        })
    }

    fn execute_withdrawal(&self, ctx: &mut LendingContext, amount: u64) -> Result<LendingOperationResult, ProgramError> {
        use pinocchio::instruction::{Seed, Signer};

        // Parse accounts
        let accounts = DriftAccounts::checked_from_accounts(
            &ctx.integration.config,
            &[], // This would need to be passed from the caller
            self.market_index,
        )?;

        // Track balances before operation
        let reserve_vault_before = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };

        let spot_market_vault_before = {
            let vault = TokenAccount::from_account_info(accounts.spot_market_vault)?;
            vault.amount()
        };

        // Execute withdrawal
        Withdraw {
            state: &accounts.state,
            user: &accounts.user,
            user_stats: &accounts.user_stats,
            authority: ctx.controller_authority,
            spot_market_vault: &accounts.spot_market_vault,
            drift_signer: &accounts.drift_signer,
            user_token_account: &accounts.reserve_vault,
            token_program: &accounts.token_program,
            remaining_accounts: &accounts.remaining_accounts,
            market_index: self.market_index,
            amount,
            reduce_only: true,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(ctx.controller_pubkey),
            Seed::from(&[ctx.controller.authority_bump]),
        ])])?;

        // Calculate deltas
        let reserve_vault_after = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };
        let reserve_delta = reserve_vault_after.saturating_sub(reserve_vault_before);

        let spot_market_vault_after = {
            let vault = TokenAccount::from_account_info(accounts.spot_market_vault)?;
            vault.amount()
        };
        let integration_delta = spot_market_vault_before.saturating_sub(spot_market_vault_after);

        // Get new balance
        let new_balance = self.get_current_balance(ctx)? - integration_delta;

        Ok(LendingOperationResult {
            integration_delta,
            reserve_delta,
            new_balance,
        })
    }

    fn sync_balance(&self, ctx: &mut LendingContext) -> Result<u64, ProgramError> {
        // Parse accounts to get spot market
        let accounts = DriftAccounts::checked_from_accounts(
            &ctx.integration.config,
            &[], // This would need to be passed from the caller
            self.market_index,
        )?;

        let spot_market = find_spot_market_account_info_by_id(&accounts.remaining_accounts, self.market_index)?;

        sync_drift_balance(
            ctx.controller,
            ctx.integration,
            ctx.integration_pubkey,
            ctx.controller_pubkey,
            ctx.controller_authority,
            ctx.mint,
            spot_market,
            accounts.user,
        )
    }

    fn get_reserve_vault(&self, _ctx: &LendingContext) -> Result<&AccountInfo, ProgramError> {
        // This is a placeholder - in practice, the reserve vault would be passed
        // from the caller or stored in the context
        Err(ProgramError::InvalidAccountData)
    }
}
