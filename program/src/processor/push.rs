use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    instructions::PushArgs, 
    integrations::{
        spl_token_external::push::process_push_spl_token_external, 
        spl_token_swap::push::process_push_spl_token_swap
    }, 
    state::{Controller, Integration, Permission, Reserve}
};


pub struct PushAccounts<'info> {
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub reserve_a: &'info AccountInfo,
    pub reserve_b: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
}


impl<'info> PushAccounts<'info> {

    pub fn from_accounts(
        accounts: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if accounts.len() < 6 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller: &accounts[0],
            authority: &accounts[1],
            permission: &accounts[2],
            integration: &accounts[3],
            reserve_a: &accounts[4],
            reserve_b: &accounts[5],
            remaining_accounts: &accounts[6..]
        };
        if ctx.controller.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if ctx.permission.owner().ne(&crate::ID) {
            msg!{"permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.integration.owner().ne(&crate::ID) {
            msg!{"integration: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.reserve_a.owner().ne(&crate::ID) {
            msg!{"reserve_a: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.reserve_a.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.reserve_b.owner().ne(&crate::ID) {
            msg!{"reserve_b: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.reserve_b.is_writable() {
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
        ctx.controller, 
    )?;

    // Load in the super permission account
    let permission = Permission::load_and_check(
        ctx.permission, 
        ctx.controller.key(), 
        ctx.authority.key()
    )?;

    // Load in the integration account 
    let mut integration = Integration::load_and_check_mut(
        ctx.integration, 
        ctx.controller.key(), 
    )?;

    // Load in the reserve account for a
    let mut reserve_a = Reserve::load_and_check_mut(
        ctx.reserve_a, 
        ctx.controller.key(), 
    )?;

    // Load in the reserve account for b (if applicable)
    let reserve_b = if ctx.reserve_a.key().ne(ctx.reserve_b.key()) {
        Some(Reserve::load_and_check_mut(
            ctx.reserve_b, 
            ctx.controller.key(), 
        )?)
    } else {
        None
    };

    match args {
        PushArgs::SplTokenExternal { .. } => {
            process_push_spl_token_external(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &ctx,
                &args
            )?;
        },
        PushArgs::SplTokenSwap { .. } => {
            process_push_spl_token_swap(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &mut reserve_b.unwrap(),
                &ctx,
                &args
            )?;
        },
        _ => return Err(ProgramError::InvalidArgument)
    }
    
    // Save the reserve and integration accounts
    integration.save(ctx.integration)?;
    reserve_a.save(ctx.reserve_a)?;
    if reserve_b.is_some() {
        reserve_b.unwrap().save(ctx.reserve_b)?;
    }


    Ok(())
}

