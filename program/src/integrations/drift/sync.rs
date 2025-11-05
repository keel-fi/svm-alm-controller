use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    error::SvmAlmControllerErrors,
    integrations::drift::{
        constants::DRIFT_PROGRAM_ID,
        cpi::UpdateSpotMarketCumulativeInterest,
        pdas::{
            derive_drift_spot_market_pda, derive_drift_spot_market_vault_pda, derive_drift_user_pda,
        },
        protocol_state::SpotMarket,
        shared_sync::sync_drift_balance,
    },
    processor::SyncIntegrationAccounts,
    state::{Controller, Integration, Reserve},
};
use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, ProgramResult};

define_account_struct! {
    pub struct SyncDriftAccounts<'info> {
        state: @owner(DRIFT_PROGRAM_ID);
        spot_market_vault: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        spot_market: mut, @owner(DRIFT_PROGRAM_ID);
        spot_market_oracle;
        user: @owner(DRIFT_PROGRAM_ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
    }
}

impl<'info> SyncDriftAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        controller_authority: &'info AccountInfo,
        accounts_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;
        let config = match config {
            IntegrationConfig::Drift(drift_config) => drift_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let spot_market_vault_pda = derive_drift_spot_market_vault_pda(config.spot_market_index)?;
        if spot_market_vault_pda.ne(ctx.spot_market_vault.key()) {
            msg! {"drift spot market vault: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let drift_spot_market_pda = derive_drift_spot_market_pda(config.spot_market_index)?;
        if drift_spot_market_pda.ne(ctx.spot_market.key()) {
            msg! {"drift spot market: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let drift_user_pda =
            derive_drift_user_pda(controller_authority.key(), config.sub_account_id)?;
        if drift_user_pda.ne(ctx.user.key()) {
            msg! {"drift user: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // Validate the Oracle matches the SpotMarket
        let spot_market_data = ctx.spot_market.try_borrow_data()?;
        let spot_market = SpotMarket::try_from_slice(&spot_market_data)?;
        if spot_market.oracle.ne(ctx.spot_market_oracle.key()) {
            msg! {"drift spot market: Invalid Oracle"}
            return Err(SvmAlmControllerErrors::InvalidAccountData.into());
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
        outer_ctx.controller_authority,
        outer_ctx.remaining_accounts,
    )?;

    // Update Drift SpotMarket interest
    UpdateSpotMarketCumulativeInterest {
        state: inner_ctx.state,
        spot_market: inner_ctx.spot_market,
        oracle: inner_ctx.spot_market_oracle,
        spot_market_vault: inner_ctx.spot_market_vault,
    }
    .invoke()?;

    // Sync liquidity value
    let new_balance = sync_drift_balance(
        controller,
        integration,
        outer_ctx.integration.key(),
        outer_ctx.controller.key(),
        outer_ctx.controller_authority,
        &reserve.mint,
        inner_ctx.spot_market,
        inner_ctx.user,
    )?;

    // Update the state
    match &mut integration.state {
        crate::enums::IntegrationState::Drift(drift_state) => {
            drift_state.balance = new_balance;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}
