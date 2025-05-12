use crate::{
    constants::ADDRESS_LOOKUP_TABLE_PROGRAM_ID,
    error::SvmAlmControllerErrors,
    events::{IntegrationUpdateEvent, SvmAlmControllerEvent},
    instructions::ManageIntegrationArgs,
    state::{Controller, Integration, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub struct ManageIntegrationAccounts<'info> {
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub lookup_table: &'info AccountInfo,
}

impl<'info> ManageIntegrationAccounts<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller: &accounts[0],
            authority: &accounts[1],
            permission: &accounts[2],
            integration: &accounts[3],
            lookup_table: &accounts[4],
        };
        if !ctx.controller.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.permission.is_owned_by(&crate::ID) {
            msg! {"permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // New Integration AccountInfo must be mutable
        if !ctx.integration.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        // The Integration AccountInfo must be the system program and be empty
        if !ctx.integration.is_owned_by(&crate::ID) {
            msg! {"integration: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // The Lookuptable must be either a value ALUT or the system_program (i.e. no LUT provided)
        if ctx.lookup_table.key().ne(&pinocchio_system::id())
            && !ctx
                .lookup_table
                .is_owned_by(&ADDRESS_LOOKUP_TABLE_PROGRAM_ID)
        {
            msg! {"lookup_table: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(ctx)
    }
}

pub fn process_manage_integration(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("manage_integration");

    let ctx = ManageIntegrationAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = ManageIntegrationArgs::try_from_slice(instruction_data).unwrap();

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    if !permission.can_manage_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Load in and check the integration
    let mut integration = Integration::load_and_check_mut(ctx.integration, ctx.controller.key())?;

    let old_state = integration.clone();

    integration.update_and_save(
        ctx.integration,
        args.status,
        if ctx.lookup_table.key().ne(&pinocchio_system::ID) {
            Some(*ctx.lookup_table.key())
        } else {
            None
        },
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
            old_state: Some(old_state),
            new_state: Some(integration),
        }),
    )?;

    Ok(())
}
