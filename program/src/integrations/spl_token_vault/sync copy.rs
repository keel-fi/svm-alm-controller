use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, 
    msg, 
    program_error::ProgramError, 
};
use crate::{
    enums::IntegrationConfig, processor::PullAccounts, state::{Controller, Integration, Permission} 
};
use pinocchio_token::{self, state::TokenAccount};


pub struct PullSplTokenVaultAccounts<'info> {
    pub mint: &'info AccountInfo,
    pub vault: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
}

impl<'info> PullSplTokenVaultAccounts<'info> {

    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            mint: &account_infos[0],
            vault: &account_infos[1],
            token_program: &account_infos[2],
        };
        let config = match config {
            IntegrationConfig::SplTokenVault(config) => config,
            _ => return Err(ProgramError::InvalidAccountData)
        };
        if ctx.token_program.key().ne(&config.program) { // TODO: Allow token 2022
            msg!{"token_program: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.mint.key().ne(&config.mint) { 
            msg!{"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.mint.owner().ne(&config.program) { 
            msg!{"mint: not owned by token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.vault.is_writable() {
            msg!{"vault: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.vault.key().ne(&config.vault) { 
            msg!{"vault: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.vault.owner().ne(&config.program) { 
            msg!{"vault: not owned by token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(ctx)
    }

}




pub fn process_pull_spl_token_vault(
    controller: &Controller,
    permission: &Permission,
    integration: &Integration,
    outer_ctx: &PullAccounts,
    instruction_data: &[u8]
) -> Result<(), ProgramError> {
    
    // SplTokenVault PULL implementation

    msg!("process_pull_spl_token_vault");

    // Check permission
    if !permission.can_reallocate() {
        return Err(ProgramError::MissingRequiredSignature)
    }

    let inner_ctx = PullSplTokenVaultAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts
    )?;

    let vault = TokenAccount::from_account_info(inner_ctx.vault).unwrap();

    

  
    // Emit accounting event
  
    
    Ok(())

}

