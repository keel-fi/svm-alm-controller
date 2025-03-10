use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    enums::IntegrationConfig, 
    integrations::spl_token_vault::sync::process_sync_spl_token_vault, 
    state::{Controller, Integration}
};


pub struct SyncAccounts<'info> {
    pub controller_info: &'info AccountInfo,
    pub integration_info: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
}

impl<'info> SyncAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller_info: &account_infos[0],
            integration_info: &account_infos[1],
            remaining_accounts: &account_infos[2..]
        };
        if ctx.controller_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.integration_info.owner().ne(&crate::ID) {
            msg!{"Integration: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(ctx)
    }
}



pub fn process_sync(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("sync");

    let ctx = SyncAccounts::from_accounts(accounts)?;
 
    // Load in controller state
    let controller = Controller::load_and_check(
        ctx.controller_info, 
    )?;

    // Load in controller state
    let mut integration = Integration::load_and_check_mut(
        ctx.integration_info,
        ctx.controller_info.key(), 
    )?;

    match integration.config {
        IntegrationConfig::SplTokenVault(_config) => { process_sync_spl_token_vault(&controller, &mut integration, &ctx)? },
        // TODO: More integration types to be supported
        _ => return Err(ProgramError::InvalidArgument)
    };
    
    Ok(())
}

