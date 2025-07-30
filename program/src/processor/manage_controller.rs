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

impl<'info> ManageControllerAccounts<'info> {
    pub fn checked_from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts)?;
        Ok(ctx)
    }
}

pub fn process_manage_controller(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("manage_controller");

    let ctx = ManageControllerAccounts::checked_from_accounts(accounts)?;
    // // Deserialize the args
    let args = ManageControllerArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Load in controller state
    let mut controller = Controller::load_and_check(ctx.controller)?;

    let old_state = controller.clone();

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;

    // Check that super authority has permission and the permission is active
    match args.status {
        ControllerStatus::Active => {
            if !permission.can_unfreeze_controller() {
                return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
            }
        }
        ControllerStatus::Suspended => {
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
