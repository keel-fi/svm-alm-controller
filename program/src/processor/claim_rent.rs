use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::PermissionStatus,
    error::SvmAlmControllerErrors,
    state::{Controller, Permission},
};

define_account_struct! {
    pub struct ClaimRentAccounts<'info> {
        controller: @owner(crate::ID);
        // controller_authority must be mutable in order to transfer its SOL
        controller_authority: mut, empty, @owner(pinocchio_system::ID);
        authority: mut, signer;
        permission: @owner(crate::ID);
        // destination must be mutable to receive sol
        destination: mut;
        system_program: @pubkey(pinocchio_system::ID);
    }
}

pub fn process_claim_rent(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!("process_claim_rent");
    let ctx = ClaimRentAccounts::from_accounts(accounts)?;

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;
    if !controller.is_active() {
        return Err(SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction.into());
    }

    // Load in the permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    if permission.status != PermissionStatus::Active {
        return Err(SvmAlmControllerErrors::PermissionStatusDoesNotPermitAction.into());
    }
    // Permission must be able to reallocate
    if !permission.can_reallocate {
        msg! {"permission: can_reallocate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    // Get current controller_authority balance
    let lamports = ctx.controller_authority.lamports();

    // If controller_authority balance is 0, throw error
    if lamports == 0 {
        msg! {"controller_authority balance bust me > 0"}
        return Err(ProgramError::InsufficientFunds);
    }

    // Transfer controller_authority balance to the destination
    Transfer {
        from: ctx.controller_authority,
        to: ctx.destination,
        lamports,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    Ok(())
}
