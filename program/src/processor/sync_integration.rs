use crate::{
    define_account_struct, enums::IntegrationConfig, integrations::{spl_token_swap::sync::process_sync_spl_token_swap, utilization_market::{config::UtilizationMarketConfig, kamino::sync::process_sync_kamino}}, state::{nova_account::NovaAccount, Controller, Integration, Reserve}
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
        controller_authority: mut, empty, @owner(pinocchio_system::ID);
        authority: signer;
        integration: mut, @owner(crate::ID);
        reserve: mut, @owner(crate::ID);
        program_id: @pubkey(crate::ID);
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
    let controller = Controller::load_and_check(ctx.controller)?;

    // Load in controller state
    let mut integration = Integration::load_and_check_mut(ctx.integration, ctx.controller.key())?;

    // Refresh the rate limits
    integration.refresh_rate_limit(clock)?;

    // Load in the reserve account (kamino only handles one reserve)
    let mut reserve = Reserve::load_and_check_mut(ctx.reserve, ctx.controller.key())?;

    // Depending on the integration, there may be an
    //  inner (integration-specific) sync logic to call
    match integration.config {
        IntegrationConfig::SplTokenSwap(_config) => {
            process_sync_spl_token_swap(&controller, &mut integration, &ctx)?
        },
        IntegrationConfig::UtilizationMarket(c) => {
            match c {
                UtilizationMarketConfig::KaminoConfig(_config) => {
                    process_sync_kamino(&controller, &mut integration, &mut reserve, &ctx)?
                }
            }
        }
        // TODO: More integration types to be supported
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Save the account
    integration.save(ctx.integration)?;

    Ok(())
}
