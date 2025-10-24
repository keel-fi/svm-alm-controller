use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    state::{keel_account::KeelAccount, Controller, Reserve},
};
use pinocchio::{account_info::AccountInfo, msg, pubkey::Pubkey, ProgramResult};

define_account_struct! {
    pub struct SyncReserveAccounts<'info> {
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        reserve: mut, @owner(crate::ID);
        vault;
    }
}

/// Sync a Reserve, updating it's balance and emitting an event
/// for accounting if the balance changed.
pub fn process_sync_reserve(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("process_sync_reserve");

    let ctx = SyncReserveAccounts::from_accounts(accounts)?;

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;
    // Error when Controller is frozen
    if controller.is_frozen() {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    // Load in the permission account
    let mut reserve = Reserve::load_and_check(ctx.reserve, ctx.controller.key())?;

    // Call the method to synchronize the reserve's state
    //  and rate limits
    reserve.sync_balance(
        ctx.vault,
        ctx.controller_authority,
        ctx.controller.key(),
        &controller,
    )?;

    // Save the state
    reserve.save(ctx.reserve)?;

    Ok(())
}
