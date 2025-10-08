use crate::{
    define_account_struct,
    enums::{IntegrationStatus, PermissionStatus, ReserveStatus},
    error::SvmAlmControllerErrors,
    instructions::PushArgs,
    integrations::{
        cctp_bridge::push::process_push_cctp_bridge, lz_bridge::push::process_push_lz_bridge,
        spl_token_external::push::process_push_spl_token_external,
        utilization_market::kamino::push::process_push_kamino,
    },
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
    pub struct PushAccounts<'info> {
        controller: @owner(crate::ID);
        controller_authority: mut, empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        integration: mut, @owner(crate::ID);
        reserve_a: mut, @owner(crate::ID);
        reserve_b: mut, @owner(crate::ID);
        program_id: @pubkey(crate::ID);
        @remaining_accounts as remaining_accounts;
    }
}

pub fn process_push(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("push");

    let clock = Clock::get()?;

    let ctx = PushAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = PushArgs::try_from_slice(instruction_data)
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

    // Load in the reserve account for b (if applicable)
    // `mut` is kept intentionally so `.as_mut()` can be used safely.
    #[allow(unused_mut)] 
    let mut reserve_b = if ctx.reserve_a.key().ne(ctx.reserve_b.key()) {
        let reserve_b = Reserve::load_and_check(ctx.reserve_b, ctx.controller.key())?;
        if reserve_b.status != ReserveStatus::Active {
            return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
        }
        Some(reserve_b)
    } else {
        None
    };

    match args {
        PushArgs::SplTokenExternal { .. } => {
            process_push_spl_token_external(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &ctx,
                &args,
            )?;
        }
        PushArgs::CctpBridge { .. } => {
            process_push_cctp_bridge(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &ctx,
                &args,
            )?;
        }
        PushArgs::LzBridge { .. } => {
            process_push_lz_bridge(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &ctx,
                &args,
            )?;
        }
        PushArgs::Kamino { .. } => {
            process_push_kamino(
                &controller, 
                &permission, 
                &mut integration, 
                &mut reserve_a,
                &ctx, 
                &args
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
