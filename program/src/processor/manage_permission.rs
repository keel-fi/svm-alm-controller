use crate::{
    define_account_struct,
    enums::PermissionStatus,
    error::SvmAlmControllerErrors,
    events::{PermissionUpdateEvent, SvmAlmControllerEvent},
    instructions::ManagePermissionArgs,
    processor::shared::verify_system_account,
    state::{Controller, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct ManagePermissionAccounts<'info> {
        payer: signer, mut;
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        super_authority: signer;
        super_permission: @owner(crate::ID);
        authority;
        permission: mut;
        program_id: @pubkey(crate::ID);
        system_program: @pubkey(pinocchio_system::ID);
    }
}

/// Logic for a Super Permission with `can_manage_permissions` to create/update
/// a Permission account.
fn manage_permission(
    ctx: &ManagePermissionAccounts,
    args: &ManagePermissionArgs,
) -> Result<(Permission, Option<Permission>), ProgramError> {
    if ctx.permission.data_is_empty() {
        // Initialize the permission account
        verify_system_account(ctx.permission, true)?;
        let permission = Permission::init_account(
            ctx.permission,
            ctx.payer,
            *ctx.controller.key(),
            *ctx.authority.key(),
            args.status,
            args.can_manage_permissions,
            args.can_invoke_external_transfer,
            args.can_execute_swap,
            args.can_reallocate,
            args.can_freeze_controller,
            args.can_unfreeze_controller,
            args.can_manage_reserves_and_integrations,
            args.can_suspend_permissions,
            args.can_liquidate,
        )?;
        Ok((permission, None))
    } else {
        // Load the permission account
        let mut permission =
            Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
        let old_state = permission.clone();
        // Update the permission account and save it
        permission.update_and_save(
            ctx.permission,
            Some(args.status),
            Some(args.can_manage_permissions),
            Some(args.can_invoke_external_transfer),
            Some(args.can_execute_swap),
            Some(args.can_reallocate),
            Some(args.can_freeze_controller),
            Some(args.can_unfreeze_controller),
            Some(args.can_manage_reserves_and_integrations),
            Some(args.can_suspend_permissions),
            Some(args.can_liquidate),
        )?;
        Ok((permission, Some(old_state)))
    }
}

/// Logic for a Permission with `can_suspend_permissions` suspend AND ONLY suspend
/// an existing permission account.
fn suspend_permission(
    ctx: &ManagePermissionAccounts,
    args: &ManagePermissionArgs,
) -> Result<(Permission, Option<Permission>), ProgramError> {
    // Check that status is suspended since that's all the permission with
    // `can_suspend_permissions` can do.
    if args.status != PermissionStatus::Suspended {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }
    // Load the permission account
    let mut permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;

    // A Permission with `can_suspend_permissions` cannot suspend Permissions
    // that can manage other permissions. This is to prevent a scenario where
    // All Permissions with management capabilities are suspended and thus no Permissions
    // could become un-suspended.
    if permission.can_manage_permissions() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    let old_state = permission.clone();
    // Update the permission account and save it
    permission.update_and_save(
        ctx.permission,
        Some(args.status),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )?;
    Ok((permission, Some(old_state)))
}

pub fn process_manage_permission(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("manage_permission");

    let ctx = ManagePermissionAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = ManagePermissionArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Don't allow a permission to suspend itself or remove it's own abilities
    // to manage permissions. This is to prevent a scenario where a Controller
    // becomes locked because all Permissions are suspended and none can manage
    // other permissions.
    if ctx.permission.key().eq(ctx.super_permission.key())
        && (args.status == PermissionStatus::Suspended || !args.can_manage_permissions)
    {
        return Err(SvmAlmControllerErrors::InvalidPermission.into());
    }

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;

    // Error when Controller is frozen
    if controller.is_frozen() {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    // Load in the permission account
    let super_permission = Permission::load_and_check(
        ctx.super_permission,
        ctx.controller.key(),
        ctx.super_authority.key(),
    )?;
    let (permission, old_state) = if super_permission.can_manage_permissions() {
        // Only super permission with `can_manage_permissions` should be able to manage the entirety of a Permission.
        manage_permission(&ctx, &args)?
    } else if super_permission.can_suspend_permissions() {
        // Permission with `can_suspend_permissions` can only suspend an existing permission.
        suspend_permission(&ctx, &args)?
    } else {
        // Permission does not have correct permissions, error
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    };

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
