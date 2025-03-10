use pinocchio::{
    account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::Pubkey, sysvars::{clock::Clock, Sysvar} 
};
use pinocchio_log::log;
use crate::{
    constants::CONTROLLER_SEED, enums::{IntegrationConfig, IntegrationState}, events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent}, processor::{shared::emit_cpi, SyncAccounts}, state::{Controller, Integration} 
};
use pinocchio_token::{self, state::TokenAccount};


pub struct PullSplTokenVaultAccounts<'info> {
    pub vault: &'info AccountInfo,
}

impl<'info> PullSplTokenVaultAccounts<'info> {

    pub fn checked_from_accounts(
        controller: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 1 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            vault: &account_infos[0],
        };
        let config = match config {
            IntegrationConfig::SplTokenVault(config) => config,
            _ => return Err(ProgramError::InvalidAccountData)
        };
        if ctx.vault.key().ne(&config.vault) { 
            msg!{"vault: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.vault.owner().ne(&config.program) { 
            msg!{"vault: not owned by token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        let vault = TokenAccount::from_account_info(ctx.vault)?;
        if vault.mint().ne(&config.mint) {
            msg!{"vault: invalid mint"};
            return Err(ProgramError::InvalidAccountData);
        }
        if vault.owner().ne(controller) {
            msg!{"vault: not owned by controller"};
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(ctx)
    }

}




pub fn process_sync_spl_token_vault(
    controller: &Controller,
    integration: &mut Integration,
    outer_ctx: &SyncAccounts,
) -> Result<(), ProgramError> {
    
    // SplTokenVault SYNC implementation

    msg!("process_sync_spl_token_vault");

    let inner_ctx = PullSplTokenVaultAccounts::checked_from_accounts(
        outer_ctx.controller_info.key(),
        &integration.config,
        outer_ctx.remaining_accounts
    )?;

    // Get the current slot and time
    let clock = Clock::get()?;
    
    // Load in the vault, since it could have an opening balance
    let vault = TokenAccount::from_account_info(inner_ctx.vault)?;

    let previous_balance: u64;
    let new_balance: u64;
    match &mut integration.state {
        IntegrationState::SplTokenVault(state) => {
            previous_balance = state.last_balance;
            new_balance = vault.amount();
            state.last_refresh_timestamp = clock.unix_timestamp;
            state.last_refresh_slot = clock.slot;
            state.last_balance = new_balance;
        },
        _ => return Err(ProgramError::InvalidAccountData.into())
    }

    let mint = match integration.config {
        IntegrationConfig::SplTokenVault(config) => { config.mint },
        _ => return Err(ProgramError::InvalidAccountData.into())
    };

    // Prevent spamming/ddos attacks -- since the sync ixn is permissionless
    //  calling this repeatedly could bombard the program and indevers
    // if new_balance == previous_balance {
    //     return Err(ProgramError::InvalidInstructionData.into())
    // }


    // Emit the accounting event
    emit_cpi(
        outer_ctx.controller_info,
        [
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller.id.to_le_bytes()),
            Seed::from(&[controller.bump])
        ],
        SvmAlmControllerEvent::AccountingEvent (
            AccountingEvent {
                controller: *outer_ctx.controller_info.key(),
                integration: *outer_ctx.integration_info.key(),
                mint: mint,
                action: AccountingAction::Sync,
                before: previous_balance,
                after: new_balance
            }
        )
    )?;

    // Save the changes to the integration account
    integration.save(&outer_ctx.integration_info)?;
  
    Ok(())

}

