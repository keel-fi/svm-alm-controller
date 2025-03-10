use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    constants::{ADDRESS_LOOKUP_TABLE_PROGRAM_ID, CONTROLLER_SEED}, enums::IntegrationType, error::SvmAlmControllerErrors, events::{IntegrationUpdateEvent, SvmAlmControllerEvent}, instructions::InitializeIntegrationArgs, integrations::spl_token_vault::initialize::process_initialize_spl_token_vault, processor::shared::emit_cpi, state::{Controller, Integration, Permission}
};


pub struct InitializeIntegrationAccounts<'info> {
    pub payer_info: &'info AccountInfo,
    pub controller_info: &'info AccountInfo,
    pub authority_info: &'info AccountInfo,
    pub permission_info: &'info AccountInfo,
    pub integration_info: &'info AccountInfo,
    pub lookup_table_info: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
}

impl<'info> InitializeIntegrationAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() < 7 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            payer_info: &account_infos[0],
            controller_info: &account_infos[1],
            authority_info: &account_infos[2],
            permission_info: &account_infos[3],
            integration_info: &account_infos[4],
            lookup_table_info: &account_infos[5],
            system_program: &account_infos[6],
            remaining_accounts: &account_infos[7..]
        };
        if !ctx.payer_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.payer_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.controller_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.authority_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if ctx.permission_info.owner().ne(&crate::ID) {
            msg!{"Permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // New Integration AccountInfo must be mutable
        if !ctx.integration_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        // The Integration AccountInfo must be the system program and be empty
        if ctx.integration_info.owner().ne(&pinocchio_system::id()) {
            msg!{"Integration: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration_info.data_is_empty() {
            msg!{"Integration: not empty"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // The Lookuptable must be either a value ALUT or the system_program (i.e. no LUT provided)
        if ctx.lookup_table_info.key().ne(&pinocchio_system::id()) && ctx.lookup_table_info.owner().ne(&ADDRESS_LOOKUP_TABLE_PROGRAM_ID) {
            msg!{"Lookup Table: wrong owner"};
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
    let args = InitializeIntegrationArgs::try_from_slice(
        instruction_data
    ).unwrap();
    
    // Load in controller state
    let controller = Controller::load_and_check(
        ctx.controller_info, 
    )?;

    // Load in the super permission account
    let permission = Permission::load_and_check(
        ctx.permission_info, 
        ctx.controller_info.key(), 
        ctx.authority_info.key()
    )?;
    // Check that super authority has permission and the permission is active
    if !permission.can_manage_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    let (config, state) = match args.integration_type {
        IntegrationType::SplTokenVault => { process_initialize_spl_token_vault(&ctx)? },
        // TODO: More integration types to be supported
        _ => return Err(ProgramError::InvalidArgument)
    };

    // Initialize the integration account
    let integration = Integration::init_account(
        ctx.integration_info, 
        ctx.payer_info, 
        *ctx.controller_info.key(),
        args.status,
        config,
        state,
        args.description,
        *ctx.lookup_table_info.key(),
    )?;
  
    msg!("just before emit-cpi");

    // Emit the Event
    emit_cpi(
        ctx.controller_info,
        [
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller.id.to_le_bytes()),
            Seed::from(&[controller.bump])
        ],
        SvmAlmControllerEvent::IntegrationUpdate (
            IntegrationUpdateEvent {
                controller: *ctx.controller_info.key(),
                integration: *ctx.integration_info.key(),
                authority: *ctx.authority_info.key(),
                old_state: None,
                new_state: Some(integration)
            }
        )
    )?;
    
    Ok(())
}

