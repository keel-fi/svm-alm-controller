// This allow is left intentionally because this instruction contains boilerplate code.
#![allow(unused_mut)]

use crate::{
    define_account_struct,
    enums::{IntegrationStatus, PermissionStatus, ReserveStatus},
    error::SvmAlmControllerErrors,
    instructions::PullArgs,
    integrations::{drift::pull::process_pull_drift, kamino::pull::process_pull_kamino},
    state::{keel_account::KeelAccount, Controller, Integration, Permission, Reserve},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

define_account_struct! {
    pub struct PullAccounts<'info> {
        controller: @owner(crate::ID);
        controller_authority: mut, empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        integration: mut, @owner(crate::ID);
        // Not all Integrations require more than 1 Reserve. Therefore, additional
        // Reserves are omitted from the outer context. It is entirely up to
        // the Integration's processor to handle additional reserves.
        reserve_a: mut, @owner(crate::ID);
        program_id: @pubkey(crate::ID);
        @remaining_accounts as remaining_accounts;
    }
}

pub fn process_pull(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("pull");

    let clock = Clock::get()?;
    let ctx = PullAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = PullArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

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

    // Load in the integration account
    let mut integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;
    if integration.status != IntegrationStatus::Active {
        return Err(SvmAlmControllerErrors::IntegrationStatusDoesNotPermitAction.into());
    }
    integration.refresh_rate_limit(clock)?;

    // Load in the reserve account for a
    let mut reserve_a = Reserve::load_and_check(ctx.reserve_a, ctx.controller.key())?;
    if reserve_a.status != ReserveStatus::Active {
        return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
    }

    match args {
        PullArgs::Kamino { .. } => {
            process_pull_kamino(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &ctx,
                &args,
            )?;
        }
        PullArgs::Drift { .. } => {
            process_pull_drift(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &ctx,
                &args,
            )?;
        }
        _ => return Err(ProgramError::InvalidArgument),
    }

    // Save the reserve and integration accounts
    integration.save(ctx.integration)?;
    reserve_a.save(ctx.reserve_a)?;

    Ok(())
}
