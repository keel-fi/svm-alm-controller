use pinocchio::{
    account_info::AccountInfo, 
    msg, 
    program_error::ProgramError, 
    pubkey::Pubkey, sysvars::{clock::Clock, Sysvar}, 
};
use crate::{
    enums::{IntegrationConfig, IntegrationState}, instructions::InitializeIntegrationArgs, integrations::spl_token_vault::{config::SplTokenVaultConfig, state::SplTokenVaultState}, processor::InitializeIntegrationAccounts
};
use pinocchio_token::{self, state::{Mint, TokenAccount}};
use pinocchio_associated_token_account::{self, instructions::CreateIdempotent};


pub struct InitializeSplTokenVaultAccounts<'info> {
    pub mint: &'info AccountInfo,
    pub vault: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
}

impl<'info> InitializeSplTokenVaultAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            mint: &account_infos[0],
            vault: &account_infos[1],
            token_program: &account_infos[2],
            associated_token_program: &account_infos[3],
        };
        if ctx.token_program.key().ne(&pinocchio_token::ID) { // TODO: Allow token 2022
            msg!{"token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.associated_token_program.key().ne(&pinocchio_associated_token_account::ID) { // TODO: Allow token 2022
            msg!{"associated_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.mint.owner().ne(ctx.token_program.key()) { // TODO: Allow token 2022
            msg!{"mint: not owned by token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.vault.is_writable() {
            msg!{"vault: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.vault.owner().ne(ctx.token_program.key()) && ctx.vault.owner().ne(&pinocchio_system::ID) {
            msg!{"vault: not owned by token_program or system_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(ctx)
    }
 

}




pub fn process_initialize_spl_token_vault(
    outer_ctx: &InitializeIntegrationAccounts,
    _outer_args: &InitializeIntegrationArgs
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_spl_token_vault");

    let inner_ctx = InitializeSplTokenVaultAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    // Load in the mint account, validating it in the process
    Mint::from_account_info(inner_ctx.mint).unwrap();

    // Invoke the CreateIdempotent ixn for the inbound_token_account (ATA)
    // Will handle both the creation or the checking, if already created
    CreateIdempotent{
        funding_account: outer_ctx.payer_info,
        account: inner_ctx.vault,
        wallet: outer_ctx.controller_info,
        mint: inner_ctx.mint,
        system_program: outer_ctx.system_program,
        token_program: inner_ctx.token_program,
    }.invoke().unwrap();

    
    // Create the Config
    let config = IntegrationConfig::SplTokenVault(
        SplTokenVaultConfig {
            program: Pubkey::from(*inner_ctx.token_program.key()),
            mint: Pubkey::from(*inner_ctx.mint.key()),
            vault: Pubkey::from(*inner_ctx.vault.key()),
            _padding: [0u8;96]
        }
    );

    // Get the current slot and time
    let clock = Clock::get()?;
    
    // Load in the vault, since it could have an opening balance
    let vault = TokenAccount::from_account_info(inner_ctx.vault)?;

    // Create the initial integration state
    let state = IntegrationState::SplTokenVault(
        SplTokenVaultState {
            last_refresh_timestamp: clock.unix_timestamp,
            last_refresh_slot: clock.slot,
            last_balance: vault.amount(),
            _padding: [0u8;24]
        }
    );

    Ok((config, state))

}

