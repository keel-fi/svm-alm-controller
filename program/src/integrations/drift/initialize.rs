use pinocchio::program_error::ProgramError;

use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::InitializeIntegrationArgs,
    integrations::drift::{config::DriftConfig, state::DriftState},
    processor::InitializeIntegrationAccounts,
};

/*
Initialize a Drift Integration. Each integration is for a specific
Mint under a specific subaccount (aka User).
 */
define_account_struct! {
  pub struct InitializeDriftAccounts<'info> {
      mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
  }
}

pub fn process_initialize_drift(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    let config = IntegrationConfig::Drift(DriftConfig {
        _padding: [0u8; 224],
    });
    let state = IntegrationState::Drift(DriftState {
        _padding: [0u8; 48],
    });

    Ok((config, state))
}
