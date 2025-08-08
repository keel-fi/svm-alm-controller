use crate::{
    define_account_struct,
    enums::{ControllerStatus, IntegrationStatus, PermissionStatus, ReserveStatus},
    error::SvmAlmControllerErrors,
    instructions::PullArgs,
    integrations::spl_token_swap::pull::process_pull_spl_token_swap,
    state::{nova_account::NovaAccount, Controller, Integration, Permission, Reserve},
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
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        integration: mut, @owner(crate::ID);
        reserve_a: mut;
        reserve_b: mut;
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
    let controller = Controller::load_and_check(ctx.controller)?;
    if controller.status != ControllerStatus::Active {
        return Err(SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction.into());
    }

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    if permission.status != PermissionStatus::Active {
        return Err(SvmAlmControllerErrors::PermissionStatusDoesNotPermitAction.into());
    }

    // Load in the super permission account
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

    // TODO [CLEANUP] Shouldn't this just return error if the reserves are 
    // equal rather than using an Option?

    // Load in the reserve account for b (if applicable)
    let reserve_b = if ctx.reserve_a.key().ne(ctx.reserve_b.key()) {
        let reserve_b = Reserve::load_and_check(ctx.reserve_b, ctx.controller.key())?;
        if reserve_b.status != ReserveStatus::Active {
            return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
        }
        Some(reserve_b)
    } else {
        None
    };

    match args {
        PullArgs::SplTokenSwap { .. } => {
            process_pull_spl_token_swap(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &mut reserve_b.unwrap(),
                &ctx,
                &args,
            )?;
        }
        _ => return Err(ProgramError::InvalidArgument),
    }

    // Save the reserve and integration accounts
    integration.save(ctx.integration)?;
    reserve_a.save(ctx.reserve_a)?;
    if reserve_b.is_some() {
        reserve_b.unwrap().save(ctx.reserve_b)?;
    }

    Ok(())
}
