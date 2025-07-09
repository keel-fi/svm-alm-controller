use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    events::{PermissionUpdateEvent, SvmAlmControllerEvent},
    instructions::ManagePermissionArgs,
    processor::shared::verify_system_account,
    state::{nova_account::NovaAccount, Controller, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct SuspendPermissionAccounts<'info> {
        payer: signer, mut;
        controller: @owner(crate::ID);
        controller_authority;
        super_authority: signer;
        super_permission: @owner(crate::ID);
        authority;
        permission: mut, @owner(crate::ID);
        program_id: @pubkey(crate::ID);
        system_program: @pubkey(pinocchio_system::ID);
    }
}

impl<'info> SuspendPermissionAccounts<'info> {
    pub fn checked_from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts)?;

        // Check that the super permission is not modifying itself.
        if ctx.permission.key().eq(ctx.super_permission.key()) {
            return Err(SvmAlmControllerErrors::InvalidPermission.into());
        }

        Ok(ctx)
    }
}

pub fn process_suspend_permission(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    //
    Ok(())
}
