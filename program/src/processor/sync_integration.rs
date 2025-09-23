use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    error::SvmAlmControllerErrors,
    integrations::spl_token_swap::sync::process_sync_spl_token_swap,
    state::{keel_account::KeelAccount, Controller, Integration},
};
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

define_account_struct! {
    pub struct SyncIntegrationAccounts<'info> {
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        integration: mut, @owner(crate::ID);
        @remaining_accounts as remaining_accounts;
    }
}

pub fn process_sync_integration(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("process_sync_integration");

    let clock = Clock::get()?;

    let ctx = SyncIntegrationAccounts::from_accounts(accounts)?;

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;
    // Error when Controller is frozen
    if controller.is_frozen() {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    // Load in integration state
    let mut integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;

    // Refresh the rate limits
    integration.refresh_rate_limit(clock)?;

    // Depending on the integration, there may be an
    //  inner (integration-specific) sync logic to call
    match integration.config {
        IntegrationConfig::SplTokenSwap(_config) => {
            process_sync_spl_token_swap(&controller, &mut integration, &ctx)?
        }
        // More integration types to be supported
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Save the account
    integration.save(ctx.integration)?;

    Ok(())
}
