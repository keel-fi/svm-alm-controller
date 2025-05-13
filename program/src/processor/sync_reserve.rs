use crate::state::{nova_account::NovaAccount, Controller, Reserve};
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub struct SyncReserveAccounts<'info> {
    pub controller: &'info AccountInfo,
    pub reserve: &'info AccountInfo,
    pub vault: &'info AccountInfo,
}

impl<'info> SyncReserveAccounts<'info> {
    pub fn from_accounts(account_infos: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if account_infos.len() != 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller: &account_infos[0],
            reserve: &account_infos[1],
            vault: &account_infos[2],
        };
        if !ctx.controller.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.reserve.is_owned_by(&crate::ID) {
            msg! {"permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.reserve.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
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
    reserve.sync_balance(ctx.vault, ctx.controller, &controller)?;

    // Save the state
    reserve.save(ctx.reserve)?;

    Ok(())
}
