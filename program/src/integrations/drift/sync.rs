use crate::{
    define_account_struct, 
    enums::IntegrationConfig,
    integrations::drift::{
        constants::DRIFT_PROGRAM_ID, 
        shared_sync::sync_drift_liquidity_value,
        protocol_state::SpotMarket
    }, 
    processor::SyncIntegrationAccounts, 
    state::{Controller, Integration, Reserve}
};
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, ProgramResult};

define_account_struct! {
    pub struct SyncDriftAccounts<'info> {
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        spot_market: @owner(DRIFT_PROGRAM_ID);
        user: @owner(DRIFT_PROGRAM_ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
    }
}

impl<'info> SyncDriftAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        accounts_infos: &'info [AccountInfo],
        reserve: &Reserve,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;
        let config = match config {
            IntegrationConfig::Drift(drift_config) => drift_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        // Validate spot market matches config
        let spot_market_data = ctx.spot_market.try_borrow_data()?;
        let spot_market_state = SpotMarket::load_checked(&spot_market_data)?;
        
        if spot_market_state.market_index != config.spot_market_index {
            msg!("spot_market_index: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        // Validate vault matches reserve
        if ctx.spot_market_vault.key().ne(&reserve.vault) {
            msg!("spot_market_vault: does not match reserve vault");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

pub fn process_sync_drift(
    controller: &Controller,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &SyncIntegrationAccounts,
) -> ProgramResult {
    msg!("process_sync_drift");
    
    let inner_ctx = SyncDriftAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
        reserve,
    )?;

    // Sync the reserve before main logic
    reserve.sync_balance(
        inner_ctx.spot_market_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // Get the drift config to extract market index
    let drift_config = match &integration.config {
        IntegrationConfig::Drift(config) => config,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    // Sync liquidity value and update state
    let (new_liquidity_value, new_deposit_amount) = sync_drift_liquidity_value(
        controller,
        integration,
        outer_ctx.integration.key(),
        outer_ctx.controller.key(),
        outer_ctx.controller_authority,
        &reserve.mint,
        inner_ctx.spot_market,
        inner_ctx.user,
        drift_config.spot_market_index,
    )?;

    // Update the state
    match &mut integration.state {
        crate::enums::IntegrationState::Drift(drift_state) => {
            drift_state.last_liquidity_value = new_liquidity_value;
            drift_state.last_deposit_amount = new_deposit_amount;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}