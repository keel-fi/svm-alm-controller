use crate::{
    error::SvmAlmControllerErrors,
    events::{ReserveUpdateEvent, SvmAlmControllerEvent},
    instructions::ManageReserveArgs,
    state::{Controller, Permission, Reserve},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub struct ManageReserveAccounts<'info> {
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub reserve: &'info AccountInfo,
}

impl<'info> ManageReserveAccounts<'info> {
    pub fn from_accounts(account_infos: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if account_infos.len() != 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller: &account_infos[0],
            authority: &account_infos[1],
            permission: &account_infos[2],
            reserve: &account_infos[3],
        };
        if !ctx.controller.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.reserve.is_owned_by(&crate::ID) {
            msg! {"reserve: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.permission.is_owned_by(&crate::ID) {
            msg! {"permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.reserve.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

pub fn process_manage_reserve(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("process_manage_reserve");

    let ctx = ManageReserveAccounts::from_accounts(accounts)?;

    let args = ManageReserveArgs::try_from_slice(instruction_data).unwrap();

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    if !permission.can_manage_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Load in the super permission account
    let mut reserve = Reserve::load_and_check_mut(ctx.reserve, ctx.controller.key())?;

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
        ctx.controller,
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
