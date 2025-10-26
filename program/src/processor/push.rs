use crate::{
    define_account_struct,
    enums::{IntegrationStatus, PermissionStatus, ReserveStatus},
    error::SvmAlmControllerErrors,
    instructions::PushArgs,
    integrations::{
        cctp_bridge::push::process_push_cctp_bridge, drift::push::process_push_drift,
        kamino::push::process_push_kamino, lz_bridge::push::process_push_lz_bridge,
        spl_token_external::push::process_push_spl_token_external,
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
        // controller_authority must to be mutable since Kamino requires the `owner`
        // to be `mut` for depositing
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

/// "Push" tokens out of a Reserve and into some downstream
/// protocol. This may be to bridge to another chain OR deposit
/// tokens into a lending protocol. We handle checks across all
/// integrations in the outer context, but leave the integration
/// specific logic to the internal processors.
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
        PushArgs::Drift { .. } => {
            process_push_drift(
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
                &args,
            )?;
        }
    }

    // Save the reserve and integration accounts
    integration.save(ctx.integration)?;
    reserve_a.save(ctx.reserve_a)?;

    Ok(())
}
