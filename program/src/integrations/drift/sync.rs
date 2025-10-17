use crate::{
    define_account_struct, integrations::drift::constants::DRIFT_PROGRAM_ID, processor::SyncIntegrationAccounts, state::{Controller, Integration, Reserve}
};
use pinocchio::ProgramResult;


define_account_struct! {
    pub struct SyncDriftAccounts<'info> {
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
    }
}

pub fn process_sync_drift(
    controller: &Controller,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &SyncIntegrationAccounts,
) -> ProgramResult {
    let inner_ctx = SyncDriftAccounts::from_accounts(&outer_ctx.remaining_accounts)?;

    // Sync the reserve before main logic
    reserve.sync_balance(
        inner_ctx.spot_market_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;


    Ok(())
}