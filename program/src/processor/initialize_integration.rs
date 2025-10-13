use crate::{
    define_account_struct,
    enums::IntegrationType,
    error::SvmAlmControllerErrors,
    events::{IntegrationUpdateEvent, SvmAlmControllerEvent},
    instructions::InitializeIntegrationArgs,
    integrations::{
        atomic_swap::initialize::process_initialize_atomic_swap, 
        cctp_bridge::initialize::process_initialize_cctp_bridge, 
        drift::initialize::process_initialize_drift,
        kamino::initialize::process_initialize_kamino, 
        lz_bridge::initialize::process_initialize_lz_bridge, 
        spl_token_external::initialize::process_initialize_spl_token_external,
    },
    state::{Controller, Integration, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct InitializeIntegrationAccounts<'info> {
        payer: signer, mut;
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        integration: mut, empty, @owner(pinocchio_system::ID);
        program_id: @pubkey(crate::ID);
        system_program: @pubkey(pinocchio_system::ID);
        @remaining_accounts as remaining_accounts;
    }
}

pub fn process_initialize_integration(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_integration");

    let ctx = InitializeIntegrationAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = InitializeIntegrationArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;

    // Error when Controller is frozen
    if controller.is_frozen() {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    // Load in the permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that authority has permission and the permission is active
    if !permission.can_manage_reserves_and_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    let (config, state) = match args.integration_type {
        IntegrationType::SplTokenExternal => process_initialize_spl_token_external(&ctx, &args)?,
        IntegrationType::CctpBridge => process_initialize_cctp_bridge(&ctx, &args)?,
        IntegrationType::LzBridge => process_initialize_lz_bridge(&ctx, &args)?,
        IntegrationType::AtomicSwap => process_initialize_atomic_swap(&ctx, &args)?,
        IntegrationType::Drift => process_initialize_drift(&ctx, &args, &controller)?,
        IntegrationType::Kamino => process_initialize_kamino(&controller, &ctx, &args)?,
        // More integration types to be supported
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Initialize the integration account
    let integration = Integration::init_account(
        ctx.integration,
        ctx.payer,
        *ctx.controller.key(),
        args.status,
        config,
        state,
        args.description,
        args.rate_limit_slope,
        args.rate_limit_max_outflow,
        args.permit_liquidation,
    )?;

    // Emit the event
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: *ctx.controller.key(),
            integration: *ctx.integration.key(),
            authority: *ctx.authority.key(),
            old_state: None,
            new_state: Some(integration),
        }),
    )?;

    Ok(())
}
