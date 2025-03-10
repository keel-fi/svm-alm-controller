use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    constants::CONTROLLER_SEED, error::SvmAlmControllerErrors, events::{PermissionUpdateEvent, SvmAlmControllerEvent}, instructions::ManagePermissionArgs, processor::shared::{emit_cpi, verify_system_account}, state::{permission, Controller, Integration, Permission}
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
    _program_id: &Pubkey,
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
    let controller = Controller::load_and_check(
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

    let mut permission: Permission;
    let old_state: Option<Permission>;
    if ctx.permission_info.data_is_empty() {
        // Initialize the permission account
        verify_system_account(ctx.permission_info, true)?;
        permission = Permission::init_account(
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
            args.can_manage_integrations,
        )?;
        old_state = None;
    } else {
        // Initialize the permission account
        permission = Permission::load_and_check_mut(
            ctx.permission_info,
            ctx.controller_info.key(),
            ctx.authority_info.key()
        )?;
        old_state = Some(permission.clone());
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
            Some(args.can_manage_integrations),
        )?;
    }
    
    // Emit the Event
    emit_cpi(
        ctx.controller_info,
        [
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller.id.to_le_bytes()),
            Seed::from(&[controller.bump])
        ],
        SvmAlmControllerEvent::PermissionUpdate (
            PermissionUpdateEvent {
                controller: *ctx.controller_info.key(),
                permission: *ctx.permission_info.key(),
                authority: *ctx.authority_info.key(),
                old_state: old_state,
                new_state: Some(permission)
            }
        )
    )?;

    Ok(())
}

