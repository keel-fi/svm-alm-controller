use crate::{
    constants::ADDRESS_LOOKUP_TABLE_PROGRAM_ID,
    enums::IntegrationType,
    error::SvmAlmControllerErrors,
    events::{IntegrationUpdateEvent, SvmAlmControllerEvent},
    instructions::InitializeIntegrationArgs,
    integrations::{
        cctp_bridge::initialize::process_initialize_cctp_bridge,
        lz_bridge::initialize::process_initialize_lz_bridge,
        spl_token_external::initialize::process_initialize_spl_token_external,
        spl_token_swap::initialize::process_initialize_spl_token_swap, swap::initialize::process_initialize_atomic_swap,
    },
    state::{Controller, Integration, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub struct InitializeIntegrationAccounts<'info> {
    pub payer: &'info AccountInfo,
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub lookup_table: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
}

impl<'info> InitializeIntegrationAccounts<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 7 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            payer: &accounts[0],
            controller: &accounts[1],
            authority: &accounts[2],
            permission: &accounts[3],
            integration: &accounts[4],
            lookup_table: &accounts[5],
            system_program: &accounts[6],
            remaining_accounts: &accounts[7..],
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
        // New Integration AccountInfo must be mutable
        if !ctx.integration.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        // The Integration AccountInfo must be the system program and be empty
        if !ctx.integration.is_owned_by(&pinocchio_system::id()) {
            msg! {"Integration: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration.data_is_empty() {
            msg! {"Integration: not empty"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // The Lookuptable must be either a value ALUT or the system_program (i.e. no LUT provided)
        if ctx.lookup_table.key().ne(&pinocchio_system::id())
            && !ctx
                .lookup_table
                .is_owned_by(&ADDRESS_LOOKUP_TABLE_PROGRAM_ID)
        {
            msg! {"Lookup Table: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.system_program.key().ne(&pinocchio_system::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(ctx)
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
    let args = InitializeIntegrationArgs::try_from_slice(instruction_data).unwrap();

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    if !permission.can_manage_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    let (config, state) = match args.integration_type {
        IntegrationType::SplTokenExternal => process_initialize_spl_token_external(&ctx, &args)?,
        IntegrationType::SplTokenSwap => process_initialize_spl_token_swap(&ctx, &args)?,
        IntegrationType::CctpBridge => process_initialize_cctp_bridge(&ctx, &args)?,
        IntegrationType::LzBridge => process_initialize_lz_bridge(&ctx, &args)?,
        IntegrationType::AtomicSwap => process_initialize_atomic_swap(&ctx, &args)?,
        // TODO: More integration types to be supported
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
        *ctx.lookup_table.key(),
        args.rate_limit_slope,
        args.rate_limit_max_outflow,
    )?;

    // Emit the event
    controller.emit_event(
        ctx.controller,
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
