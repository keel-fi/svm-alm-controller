use crate::{
    define_account_struct,
    state::{nova_account::NovaAccount, Controller, Reserve},
};
use pinocchio::{account_info::AccountInfo, msg, pubkey::Pubkey, ProgramResult};

define_account_struct! {
    pub struct SyncReserveAccounts<'info> {
        controller: @owner(crate::ID);
        controller_authority;
        reserve: mut, @owner(crate::ID);
        vault;
    }
}

pub fn process_sync_reserve(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("process_sync_reserve");

    let ctx = SyncReserveAccounts::from_accounts(accounts)?;

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;

    // Load in the super permission account
    let mut reserve = Reserve::load_and_check_mut(ctx.reserve, ctx.controller.key())?;

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
