use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    events::{ReserveUpdateEvent, SvmAlmControllerEvent},
    instructions::ManageReserveArgs,
    state::{keel_account::KeelAccount, Controller, Permission, Reserve},
};
use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};

define_account_struct! {
    pub struct ManageReserveAccounts<'info> {
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        reserve: mut, @owner(crate::ID);
        program_id: @pubkey(crate::ID);
    }
}

pub fn process_manage_reserve(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("process_manage_reserve");

    let ctx = ManageReserveAccounts::from_accounts(accounts)?;

    let args = ManageReserveArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;

    // Error when Controller is frozen
    if controller.is_frozen() {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    // Load in the permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that authority has permission and the permission is active
    if !permission.can_manage_reserves_and_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Load in the Reserve
    let mut reserve = Reserve::load_and_check(ctx.reserve, ctx.controller.key())?;

    // Clone the old state for emitting
    let old_state = reserve.clone();

    // Update the reserve configuration
    reserve.update(
        args.status,
        args.rate_limit_slope,
        args.rate_limit_max_outflow,
    )?;

    // Emit the Event to record the update
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::ReserveUpdate(ReserveUpdateEvent {
            controller: *ctx.controller.key(),
            reserve: *ctx.reserve.key(),
            authority: *ctx.authority.key(),
            old_state: Some(old_state),
            new_state: Some(reserve),
        }),
    )?;

    // Save the reserve state
    reserve.save(ctx.reserve)?;

    Ok(())
}
