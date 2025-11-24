use crate::{
    constants::KEEL_DEPLOYER_MSIG,
    define_account_struct,
    enums::{ControllerStatus, PermissionStatus},
    error::SvmAlmControllerErrors,
    events::{ControllerUpdateEvent, PermissionUpdateEvent, SvmAlmControllerEvent},
    instructions::InitializeControllerArgs,
    state::{Controller, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct InitializeControllerAccounts<'info> {
        payer: signer, mut;
        authority: signer;
        controller: mut, empty, @owner(pinocchio_system::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        permission: mut, empty, @owner(pinocchio_system::ID);
        program_id: @pubkey(crate::ID);
        system_program: @pubkey(pinocchio_system::ID);
    }
}

/// Initialize a Controller.
/// This is permissionless and sets up
/// a Controller with a new Super Permission.
pub fn process_initialize_controller(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_controller");

    let ctx = InitializeControllerAccounts::from_accounts(accounts)?;

    // Instruction is permissioned by the Keel multisig
    if ctx.authority.key().ne(&KEEL_DEPLOYER_MSIG) {
        msg!("authority: Invalid authority for initializing pool");
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Deserialize the args
    let args = InitializeControllerArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // AtomicSwapLock is not a valid controller status at initialization.
    if args.status == ControllerStatus::AtomicSwapLock {
        return Err(ProgramError::InvalidArgument);
    }

    // Initialize the controller data
    let controller = Controller::init_account(
        ctx.controller,
        ctx.controller_authority,
        ctx.payer,
        args.id,
        args.status,
    )?;

    // Initialize the Controller's super Permission account.
    let permission = Permission::init_account(
        ctx.permission,
        ctx.payer,
        *ctx.controller.key(),
        *ctx.authority.key(),
        PermissionStatus::Active,
        true, // Only can manage permissions to begin with
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false, // can_liquidate
    )?;

    // Emit the event for controller
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::ControllerUpdate(ControllerUpdateEvent {
            controller: *ctx.controller.key(),
            authority: *ctx.authority.key(),
            old_state: None,
            new_state: Some(controller),
        }),
    )?;

    // Emit the event for permission
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::PermissionUpdate(PermissionUpdateEvent {
            controller: *ctx.controller.key(),
            permission: *ctx.permission.key(),
            authority: *ctx.authority.key(),
            old_state: None,
            new_state: Some(permission),
        }),
    )?;

    Ok(())
}
