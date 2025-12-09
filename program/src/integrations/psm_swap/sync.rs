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
    integrations::psm_swap::{constants::PSM_SWAP_PROGRAM_ID, psm_swap_state::Token, shared_sync::sync_psm_liquidity_supplied},
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
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;
        let config = match config {
            IntegrationConfig::PsmSwap(psm_swap_config) => psm_swap_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        // Call config.check_accounts to verify accounts match config
        config.check_accounts(&ctx.psm_token, &ctx.psm_pool, &ctx.mint)?;

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
pub fn process_sync_psm_swap(
    controller: &Controller,
    integration: &mut Integration,
    outer_ctx: &SyncIntegrationAccounts,
) -> Result<(), ProgramError> {
    msg!("process_sync_psm_swap");
    let inner_ctx = SyncPsmSwapAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    
    let new_liquidity_supplied = sync_psm_liquidity_supplied(
        controller,
        integration,
        outer_ctx.controller.key(),
        outer_ctx.integration.key(),
        inner_ctx.mint.key(),
        outer_ctx.controller_authority,
        inner_ctx.psm_token_vault,
    )?;

    // Update the integration state with new liquidity_supplied
    match &mut integration.state {
        IntegrationState::PsmSwap(state) => {
            state.liquidity_supplied = new_liquidity_supplied;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    Ok(())
}
