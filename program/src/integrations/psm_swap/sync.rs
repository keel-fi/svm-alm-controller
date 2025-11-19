use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    integrations::psm_swap::{constants::PSM_SWAP_PROGRAM_ID, psm_swap_state::Token},
    processor::SyncIntegrationAccounts,
    state::{Controller, Integration, Reserve},
};

define_account_struct! {
    pub struct SyncPsmSwapAccounts<'info> {
        psm_token_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        psm_token: @owner(PSM_SWAP_PROGRAM_ID);
        psm_pool: @owner(PSM_SWAP_PROGRAM_ID);
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
    }
}

impl<'info> SyncPsmSwapAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        accounts_infos: &'info [AccountInfo],
        reserve: &Reserve,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;
        let config = match config {
            IntegrationConfig::PsmSwap(psm_swap_config) => psm_swap_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        // Call config.check_accounts to verify accounts match config
        config.check_accounts(&ctx.psm_token, &ctx.psm_pool, &ctx.mint)?;

        // Check for incorrect mint/reserve - verify reserve.mint matches config.mint
        if ctx.mint.key().ne(&reserve.mint) {
            msg!("mint: does not match reserve mint");
            return Err(ProgramError::InvalidAccountData);
        }

        // Get the psm_token state to verify vault matches
        let psm_token_data = ctx.psm_token.try_borrow_data()?;
        let psm_token =
            Token::try_from_slice(&psm_token_data).map_err(|_| ProgramError::InvalidAccountData)?;

        // Verify that the reserve_vault passed in is the PSM token vault
        // The PSM token vault is where the actual liquidity is held
        if psm_token.vault.ne(ctx.psm_token_vault.key()) {
            msg!("psm_token.vault: does not match reserve_vault");
            return Err(ProgramError::InvalidAccountData);
        }

        // Verify psm_token belongs to the pool
        if psm_token.pool.ne(ctx.psm_pool.key()) {
            msg!("psm_token: does not belong to the pool provided");
            return Err(ProgramError::InvalidAccountData);
        }

        // Verify psm_token mint matches the provided mint
        if psm_token.mint.ne(ctx.mint.key()) {
            msg!("psm_token.mint: does not match the provided mint");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

/// This function syncs a `PsmSwapIntegration`. It:
/// - Verifies that the correct mint/reserve are supplied
/// - Calls config.check_accounts to validate accounts
/// - Checks for stale values and reverts if inconsistent
/// - If tokens were transferred (inflow detected), updates both reserve and integration for inflow
pub fn process_sync_psm_swap(
    _controller: &Controller,
    integration: &mut Integration,
    _reserve: &mut Reserve,
    outer_ctx: &SyncIntegrationAccounts,
) -> Result<(), ProgramError> {
    msg!("process_sync_psm_swap");
    let inner_ctx = SyncPsmSwapAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
        _reserve,
    )?;

    // Get the current vault balance
    let current_psm_token_vault_balance =
        TokenAccount::from_account_info(&inner_ctx.psm_token_vault)?.amount();

    // Calculate the new liquidity_supplied based on current vault balance
    let new_liquidity_supplied = current_psm_token_vault_balance;

    // Check if there was an inflow by comparing current balance with previous integration state
    let previous_liquidity_supplied = match integration.state {
        IntegrationState::PsmSwap(state) => state.liquidity_supplied,
        _ => {
            return Err(ProgramError::InvalidAccountData);
        }
    };

    if current_psm_token_vault_balance > previous_liquidity_supplied {
        let inflow_delta = current_psm_token_vault_balance.saturating_sub(previous_liquidity_supplied);

        // Update integration rate limits for inflow
        // This ensures that if tokens were transferred (rewards harvested), the integration
        // is updated for inflow.
        let clock = Clock::get()?;
        integration.update_rate_limit_for_inflow(clock, inflow_delta)?;
    }

    // Update the integration state with new liquidity_supplied
    match &mut integration.state {
        IntegrationState::PsmSwap(state) => {
            state.liquidity_supplied = new_liquidity_supplied;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    Ok(())
}
