use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, sysvars::{Sysvar, clock::Clock}, ProgramResult
};
use crate::{
    enums::IntegrationConfig, 
    integrations::spl_token_swap::sync::process_sync_spl_token_swap, 
    state::{Controller, Integration}
};


pub struct SyncIntegrationAccounts<'info> {
    pub controller: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
}

impl<'info> SyncIntegrationAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller: &account_infos[0],
            integration: &account_infos[1],
            remaining_accounts: &account_infos[2..]
        };
        if !ctx.controller.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration.is_owned_by(&crate::ID) {
            msg!{"integration: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(ctx)
    }
}



pub fn process_sync_integration(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("process_sync_integration");

    let clock = Clock::get()?;

    let ctx = SyncIntegrationAccounts::from_accounts(accounts)?;
 
    // Load in controller state
    let controller = Controller::load_and_check(
        ctx.controller, 
    )?;

    // Load in controller state
    let mut integration = Integration::load_and_check_mut(
        ctx.integration,
        ctx.controller.key(), 
    )?;

    // Refresh the rate limits
    integration.refresh_rate_limit(clock)?;

    // Depending on the integration, there may be an 
    //  inner (integration-specific) sync logic to call
    match integration.config {
        IntegrationConfig::SplTokenSwap(_config) => { 
            process_sync_spl_token_swap(&controller, &mut integration, &ctx)? 
        },
        // TODO: More integration types to be supported
        _ => return Err(ProgramError::InvalidArgument)
    };
 
    // Save the account
    integration.save(ctx.integration)?;
    
    Ok(())
}

