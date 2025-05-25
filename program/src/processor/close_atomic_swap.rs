use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use crate::{
    error::SvmAlmControllerErrors,
    state::{Integration, Permission},
};

pub struct CloseAtomicSwap<'info> {
    pub payer: &'info AccountInfo,
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> CloseAtomicSwap<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            payer: &accounts[0],
            controller: &accounts[1],
            authority: &accounts[2],
            permission: &accounts[3],
            integration: &accounts[4],
            system_program: &accounts[5],
        };

        if !ctx.payer.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.payer.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.controller.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.permission.is_owned_by(&crate::ID) {
            msg! {"Permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.system_program.key().ne(&pinocchio_system::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(ctx)
    }
}

pub fn process_close_atomic_swap(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!("close_atomic_swap");
    let ctx = CloseAtomicSwap::from_accounts(accounts)?;

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    if !permission.can_manage_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Check that Integration account is valid and matches controller.
    let _integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;

    // Close account and transfer rent to payer.
    let payer_lamports = ctx.payer.lamports();
    *ctx.payer.try_borrow_mut_lamports().unwrap() = payer_lamports
        .checked_add(ctx.integration.lamports())
        .unwrap();
    *ctx.integration.try_borrow_mut_lamports().unwrap() = 0;
    ctx.integration.close()?;

    Ok(())
}
