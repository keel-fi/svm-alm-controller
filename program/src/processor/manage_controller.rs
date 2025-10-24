use crate::{
    define_account_struct,
    enums::ControllerStatus,
    error::SvmAlmControllerErrors,
    events::{ControllerUpdateEvent, SvmAlmControllerEvent},
    instructions::ManageControllerArgs,
    state::{Controller, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct ManageControllerAccounts<'info> {
        controller: mut, @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        program_id: @pubkey(crate::ID);
    }
}

/// Change a Controller's status.
/// Only authorities with a Permission
/// that has the `can_manage_reserves_and_integrations`
/// privilege may execute this instruction.
pub fn process_manage_controller(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("manage_controller");

    let ctx = ManageControllerAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = ManageControllerArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Load in controller state
    let mut controller =
        Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;

    // Error when Controller is frozen and updated status is not Active
    if controller.is_frozen() && args.status != ControllerStatus::Active {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    let old_state = controller.clone();

    // Load in the permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;

    // Check that authority has permission and the permission is active
    match args.status {
        ControllerStatus::Active => {
            if !permission.can_unfreeze_controller() {
                return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
            }
        }
        ControllerStatus::PushPullFrozen | ControllerStatus::Frozen => {
            if !permission.can_freeze_controller() {
                return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
            }
        }
    }

    // Update the controller with the new status
    controller.update_and_save(ctx.controller, args.status)?;

    // Emit the event
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::ControllerUpdate(ControllerUpdateEvent {
            controller: *ctx.controller.key(),
            authority: *ctx.authority.key(),
            old_state: Some(old_state),
            new_state: Some(controller),
        }),
    )?;

    Ok(())
}
