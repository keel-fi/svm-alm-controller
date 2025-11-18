use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError};
use pinocchio_token_interface::TokenAccount;

use crate::{
    define_account_struct, 
    enums::{IntegrationConfig, IntegrationState}, 
    instructions::InitializeIntegrationArgs, 
    integrations::psm_swap::{
        config::PsmSwapConfig, 
        constants::PSM_SWAP_PROGRAM, 
        psm_swap_state::{Token, PsmPool}, 
        state::PsmSwapState
    }, 
    processor::InitializeIntegrationAccounts
};

define_account_struct! {
    pub struct InitializePsmSwapAccounts<'info> {
        psm_pool: @owner(PSM_SWAP_PROGRAM);
        psm_token: @owner(PSM_SWAP_PROGRAM);
        psm_token_vault: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
    }
}

impl<'info> InitializePsmSwapAccounts<'info>{
    pub fn checked_from_accounts(
        account_infos: &'info [AccountInfo],
        controller_authority: &'info AccountInfo,
    ) -> Result<Self, ProgramError> {
        let ctx = InitializePsmSwapAccounts::from_accounts(account_infos)?;

        let psm_token_data = ctx.psm_token.try_borrow_data()?;
        let psm_token = Token::try_from_slice(&psm_token_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        let psm_pool_data = ctx.psm_pool.try_borrow_data()?;
        let psm_pool = PsmPool::try_from_slice(&psm_pool_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Validate the controller_authority is pool.liquidity_owner
        if psm_pool.liquidity_owner.as_ref() != controller_authority.key().as_ref() {
            msg!("psm_pool: liquidity owner does not match controller_authority");
            return Err(ProgramError::InvalidAccountData);
        }

        if psm_token.pool.ne(ctx.psm_pool.key()) {
            msg!("psm_token: psm_token does not belong to the pool provided");
            return Err(ProgramError::InvalidAccountData);
        }

        if psm_token.mint.ne(ctx.mint.key()) {
            msg!("psm_token: does not match the provided mint");
            return Err(ProgramError::InvalidAccountData);
        }

        if psm_token.vault.ne(ctx.psm_token_vault.key()) {
            msg!("psm_token_vault: does not match the provided psm_token");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

pub fn process_initialize_psm_swap(
    outer_ctx: &InitializeIntegrationAccounts,
    _outer_args: &InitializeIntegrationArgs
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_psm_swap");

    let inner_ctx 
        = InitializePsmSwapAccounts::checked_from_accounts(outer_ctx.remaining_accounts, outer_ctx.controller_authority)?;

    // load the psm_token Vault, since it could have an opening balance
    let liquidity_vault = TokenAccount::from_account_info(inner_ctx.psm_token_vault)?;
    let vault_balance = liquidity_vault.amount();

    // Create the Config
    let config = IntegrationConfig::PsmSwap(PsmSwapConfig {
        psm_token: *inner_ctx.psm_token.key(),
        psm_pool: *inner_ctx.psm_pool.key(),
        mint: *inner_ctx.mint.key(),
        _padding: [0; 128]
    });

    // Create the initial integration state
    let state = IntegrationState::PsmSwap(PsmSwapState {
        liquidity_supplied: vault_balance,
        _padding: [0; 40]
    });

    Ok((config, state))
}