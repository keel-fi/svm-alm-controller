use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    instructions::PushArgs, 
    integrations::{spl_token_external::push::process_push_spl_token_external, spl_token_swap::push::process_push_spl_token_swap}, 
    state::{Controller, Integration, Permission}
};


pub struct PushAccounts<'info> {
    pub controller_info: &'info AccountInfo,
    pub authority_info: &'info AccountInfo,
    pub permission_info: &'info AccountInfo,
    pub integration_info: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
}


impl<'info> PushAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() < 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller_info: &account_infos[0],
            authority_info: &account_infos[1],
            permission_info: &account_infos[2],
            integration_info: &account_infos[3],
            remaining_accounts: &account_infos[4..]
        };
        if ctx.controller_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.authority_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if ctx.permission_info.owner().ne(&crate::ID) {
            msg!{"Permission: wrong owner"};
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



pub fn process_push(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("push");

    let ctx = PushAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = PushArgs::try_from_slice(
        instruction_data
    ).unwrap();
    
    // Load in controller state
    let controller = Controller::load_and_check(
        ctx.controller_info, 
    )?;

    // Load in the super permission account
    let permission = Permission::load_and_check(
        ctx.permission_info, 
        ctx.controller_info.key(), 
        ctx.authority_info.key()
    )?;

    // Load in the integration account 
    let mut integration = Integration::load_and_check(
        ctx.integration_info, 
        ctx.controller_info.key(), 
    )?;
    
    match args {
        PushArgs::SplTokenExternal { .. } => {
            process_push_spl_token_external(
                &controller,
                &permission,
                &mut integration,
                &ctx,
                &args
            )?;
        },
        PushArgs::SplTokenSwap { .. } => {
            process_push_spl_token_swap(
                &controller,
                &permission,
                &mut integration,
                &ctx,
                &args
            )?;
        },
        _ => return Err(ProgramError::InvalidArgument)
    }
    
    
    Ok(())
}

