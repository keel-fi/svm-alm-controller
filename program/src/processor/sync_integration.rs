use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    error::SvmAlmControllerErrors,
    integrations::{
        drift::sync::process_sync_drift,
        kamino::sync::process_sync_kamino,
        psm_swap::sync::process_sync_psm_swap,
    },
    state::{keel_account::KeelAccount, Controller, Integration, Reserve},
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
        // controller_authority must be mutable since Kamino requires the `owner`
        // to be `mut` for depositing
        controller_authority: mut, empty, @owner(pinocchio_system::ID);
        payer: mut, signer;
        integration: mut, @owner(crate::ID);
        // Reserve required for Kamino integration sync where
        // rewards are harvested. We must "sync" the Reserve
        // prior to receiving rewards to ensure accurate accounting.
        reserve: mut, @owner(crate::ID);
        @remaining_accounts as remaining_accounts;
    }
}

/// Sync an Integration's state with the downstream protocol.
/// For a lending integration (i.e. Kamino), we calculate
/// the amount of tokens that are held as a result of previous
/// deposits and interest accrued over time. Other Integration types
/// may have different state.
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
        IntegrationConfig::Kamino(_kamino_config) => {
            // Load in the reserve account (kamino only handles one reserve)
            let mut reserve = Reserve::load_and_check(ctx.reserve, ctx.controller.key())?;

            process_sync_kamino(&controller, &mut integration, &mut reserve, &ctx)?;

            // TODO: reserve will be moved into sync_kamino inner_ctx
            // since it's the only integration that needs it for now.
            reserve.save(ctx.reserve)?;
        }
        IntegrationConfig::Drift(_config) => {
            process_sync_drift(&controller, &mut integration, &ctx)?;
        }
        IntegrationConfig::PsmSwap(_config) => {
            // Load in the reserve account (psm_swap requires a reserve)
            let mut reserve = Reserve::load_and_check(ctx.reserve, ctx.controller.key())?;

            process_sync_psm_swap(&controller, &mut integration, &mut reserve, &ctx)?;

            reserve.save(ctx.reserve)?;
        }
        // TODO: More integration types to be supported
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Save the account
    integration.save(ctx.integration)?;

    Ok(())
}
