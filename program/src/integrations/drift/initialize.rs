use pinocchio::instruction::Seed;
use pinocchio::msg;
use pinocchio::sysvars::rent::RENT_ID;
use pinocchio::{instruction::Signer, program_error::ProgramError};

use crate::account_utils::account_is_uninitialized;
use crate::constants::CONTROLLER_AUTHORITY_SEED;
use crate::instructions::InitializeArgs;
use crate::integrations::drift::cpi::InitializeUser;
use crate::integrations::drift::protocol_state::SpotMarket;
use crate::state::Controller;
use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::InitializeIntegrationArgs,
    integrations::drift::{
        config::DriftConfig, constants::DRIFT_PROGRAM_ID, cpi::InitializeUserStats,
        state::DriftState,
    },
    processor::InitializeIntegrationAccounts,
};

/*
Initialize a Drift Integration. Each integration is for a specific
Mint under a specific subaccount (aka User).
 */
define_account_struct! {
  pub struct InitializeDriftAccounts<'info> {
      drift_user: mut;
      drift_user_stats: mut;
      drift_state: mut, @owner(DRIFT_PROGRAM_ID);
      drift_spot_market: @owner(DRIFT_PROGRAM_ID);
      rent: @pubkey(RENT_ID);
      drift_program: @pubkey(DRIFT_PROGRAM_ID);
  }
}

pub fn process_initialize_drift(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
    controller: &Controller,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    let inner_ctx = InitializeDriftAccounts::from_accounts(outer_ctx.remaining_accounts)?;
    let (sub_account_id, spot_market_index) = match outer_args.inner_args {
        InitializeArgs::Drift {
            sub_account_id,
            spot_market_index,
        } => (sub_account_id, spot_market_index),
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Check that the spot_market_index is valid and matches a Drift SpotMarket
    let spot_market_data = inner_ctx.drift_spot_market.try_borrow_data()?;
    let spot_market = SpotMarket::load_checked(&spot_market_data)?;
    if spot_market.market_index != spot_market_index {
        msg!("spot_market: Invalid market index");
        return Err(ProgramError::InvalidAccountData);
    }

    // Initialize UserStats when it does not exist
    if account_is_uninitialized(inner_ctx.drift_user_stats) {
        InitializeUserStats {
            user_stats: inner_ctx.drift_user_stats,
            state: inner_ctx.drift_state,
            authority: outer_ctx.controller_authority,
            payer: outer_ctx.payer,
            rent: inner_ctx.rent,
            system_program: outer_ctx.system_program,
        }
        .invoke_signed(Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ]))?;
    }

    // Initialize Drift User when it does not exist
    if account_is_uninitialized(inner_ctx.drift_user) {
        InitializeUser {
            user: inner_ctx.drift_user,
            user_stats: inner_ctx.drift_user_stats,
            state: inner_ctx.drift_state,
            authority: outer_ctx.controller_authority,
            payer: outer_ctx.payer,
            rent: inner_ctx.rent,
            system_program: outer_ctx.system_program,
        }
        .invoke_signed(
            sub_account_id,
            Signer::from(&[
                Seed::from(CONTROLLER_AUTHORITY_SEED),
                Seed::from(outer_ctx.controller.key()),
                Seed::from(&[controller.authority_bump]),
            ]),
        )?;
    }

    let config = IntegrationConfig::Drift(DriftConfig {
        sub_account_id,
        spot_market_index,
        _padding: [0u8; 220],
    });
    let state = IntegrationState::Drift(DriftState {
        _padding: [0u8; 48],
    });

    Ok((config, state))
}
