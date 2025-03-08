use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    error::SvmAlmControllerErrors,
    instructions::ManagePermissionArgs, 
    processor::shared::{verify_signer, verify_system_account, verify_system_program}, 
    state::{Controller, Permission}
};


pub struct ManagePermissionAccounts<'info> {
    pub payer_info: &'info AccountInfo,
    pub controller_info: &'info AccountInfo,
    pub super_authority_info: &'info AccountInfo,
    pub super_permission_info: &'info AccountInfo,
    pub authority_info: &'info AccountInfo,
    pub permission_info: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> ManagePermissionAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 7 {
            return Err(ProgramError::NotEnoughAccountKeys)
        }
        let ctx = Self {
            payer_info: &account_infos[0],
            controller_info: &account_infos[1],
            super_authority_info: &account_infos[2],
            super_permission_info: &account_infos[3],
            authority_info: &account_infos[4],
            permission_info: &account_infos[5],
            system_program: &account_infos[6],
        };
        if !ctx.payer_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.payer_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.controller_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.super_authority_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if ctx.super_permission_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.permission_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !(ctx.permission_info.owner().eq(&pinocchio_system::id()) && !ctx.permission_info.data_is_empty()) && ctx.super_permission_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner)
        }
        if ctx.system_program.key().ne(&pinocchio_system::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(ctx)
    }
}



pub fn process_manage_permission(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("manage_permission");

    let ctx = ManagePermissionAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = ManagePermissionArgs::try_from_slice(
        instruction_data
    ).unwrap();
    
    // Load in controller state
    Controller::load_and_check(
        ctx.controller_info, 
    )?;

    // Load in the super permission account
    let super_permission = Permission::load_and_check(
        ctx.super_permission_info, 
        ctx.controller_info.key(), 
        ctx.super_authority_info.key()
    )?;
    // Check that super authority has permission and the permission is active
    if !super_permission.can_manage_permissions() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into())
    }

    if ctx.permission_info.data_is_empty() {
        // Initialize the permission account
        verify_system_account(ctx.permission_info, true)?;
        Permission::init_account(
            ctx.permission_info, 
            ctx.payer_info, 
            *ctx.controller_info.key(),
            *ctx.authority_info.key(),
            args.status,
            args.can_manage_permissions,
            args.can_invoke_external_transfer,
            args.can_execute_swap,
            args.can_reallocate,
            args.can_freeze,
            args.can_unfreeze,
        )?;
    } else {
        // Initialize the permission account
        let mut permission = Permission::load_and_check_mut(
            ctx.permission_info,
            ctx.controller_info.key(),
            ctx.authority_info.key()
        )?;
        // Update the permission account and save it
        permission.update_and_save(
            ctx.permission_info,
            Some(args.status),
            Some(args.can_manage_permissions),
            Some(args.can_invoke_external_transfer),
            Some(args.can_execute_swap),
            Some(args.can_reallocate),
            Some(args.can_freeze),
            Some(args.can_unfreeze),
        )?;
    }
    
    Ok(())
}

