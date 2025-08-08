use crate::{
    constants::ADDRESS_LOOKUP_TABLE_PROGRAM_ID,
    define_account_struct,
    error::SvmAlmControllerErrors,
    events::{IntegrationUpdateEvent, SvmAlmControllerEvent},
    instructions::ManageIntegrationArgs,
    state::{Controller, Integration, Permission},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct ManageIntegrationAccounts<'info> {
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        integration: mut, @owner(crate::ID);
        lookup_table;
        program_id: @pubkey(crate::ID);
    }
}

impl<'info> ManageIntegrationAccounts<'info> {
    pub fn checked_from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts)?;
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

    let ctx = ManageIntegrationAccounts::checked_from_accounts(accounts)?;
    // // Deserialize the args
    let args = ManageIntegrationArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

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
        ctx.controller_authority,
        ctx.controller.key(),
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
