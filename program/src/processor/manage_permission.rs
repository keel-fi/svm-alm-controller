use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    events::{PermissionUpdateEvent, SvmAlmControllerEvent},
    instructions::ManagePermissionArgs,
    processor::shared::verify_system_account,
    state::{nova_account::NovaAccount, Controller, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct ManagePermissionAccounts<'info> {
        payer: signer, mut;
        controller: @owner(crate::ID);
        controller_authority;
        super_authority: signer;
        super_permission: @owner(crate::ID);
        authority;
        permission: mut;
        program_id: @pubkey(crate::ID);
        system_program: @pubkey(pinocchio_system::ID);
    }
}

impl<'info> ManagePermissionAccounts<'info> {
    pub fn checked_from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts)?;
        if !(ctx.permission.is_owned_by(&pinocchio_system::id()) && !ctx.permission.data_is_empty())
            && !ctx.super_permission.is_owned_by(&crate::ID)
        {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Check that the super permission is not modifying itself.
        if ctx.permission.key().eq(ctx.super_permission.key()) {
            return Err(SvmAlmControllerErrors::InvalidPermission.into());
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

    let ctx = ManagePermissionAccounts::checked_from_accounts(accounts)?;
    // // Deserialize the args
    let args = ManagePermissionArgs::try_from_slice(instruction_data).unwrap();

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;

    // Load in the super permission account
    let super_permission = Permission::load_and_check(
        ctx.super_permission,
        ctx.controller.key(),
        ctx.super_authority.key(),
    )?;
    // Check that super authority has permission and the permission is active
    if !super_permission.can_manage_permissions() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    let mut permission: Permission;
    let old_state: Option<Permission>;
    if ctx.permission.data_is_empty() {
        // Initialize the permission account
        verify_system_account(ctx.permission, true)?;
        permission = Permission::init_account(
            ctx.permission,
            ctx.payer,
            *ctx.controller.key(),
            *ctx.authority.key(),
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
            ctx.permission,
            ctx.controller.key(),
            ctx.authority.key(),
        )?;
        old_state = Some(permission.clone());
        // Update the permission account and save it
        permission.update_and_save(
            Some(args.status),
            Some(args.can_manage_permissions),
            Some(args.can_invoke_external_transfer),
            Some(args.can_execute_swap),
            Some(args.can_reallocate),
            Some(args.can_freeze),
            Some(args.can_unfreeze),
            Some(args.can_manage_integrations),
        )?;
        // Save the state to the account
        permission.save(ctx.permission)?;
    }

    // Emit the event
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::PermissionUpdate(PermissionUpdateEvent {
            controller: *ctx.controller.key(),
            permission: *ctx.permission.key(),
            authority: *ctx.authority.key(),
            old_state: old_state,
            new_state: Some(permission),
        }),
    )?;

    Ok(())
}
