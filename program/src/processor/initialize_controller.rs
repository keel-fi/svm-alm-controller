use crate::{
    define_account_struct,
    enums::PermissionStatus,
    events::{ControllerUpdateEvent, SvmAlmControllerEvent},
    instructions::InitializeControllerArgs,
    state::{Controller, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};

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

pub fn process_initialize_controller(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_controller");

    let ctx = InitializeControllerAccounts::from_accounts(accounts)?;

    // // Deserialize the args
    let args = InitializeControllerArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Initialize the controller data
    let controller = Controller::init_account(
        ctx.controller,
        ctx.controller_authority,
        ctx.payer,
        args.id,
        args.status,
    )?;

    // Initialize the Controller's super Permission account.
    Permission::init_account(
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

    // Emit the event
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

    Ok(())
}
