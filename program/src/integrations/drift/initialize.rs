use pinocchio::instruction::Seed;
use pinocchio::sysvars::rent::RENT_ID;
use pinocchio::{instruction::Signer, program_error::ProgramError};

use crate::constants::CONTROLLER_AUTHORITY_SEED;
use crate::state::Controller;
use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::InitializeIntegrationArgs,
    integrations::drift::{
        config::DriftConfig,
        constants::DRIFT_PROGRAM_ID,
        cpi::{initialize_user_stats, InitializeUserStats},
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
      drift_user_stats;
      drift_state: mut, @owner(DRIFT_PROGRAM_ID);
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
    let config = IntegrationConfig::Drift(DriftConfig {
        _padding: [0u8; 224],
    });
    let state = IntegrationState::Drift(DriftState {
        _padding: [0u8; 48],
    });

    // TODO check if UserStats exists to skip CPI
    // Initialize UserStats
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

    // TODO init User if does not exist

    Ok((config, state))
}
