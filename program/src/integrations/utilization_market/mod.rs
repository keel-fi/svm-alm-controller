pub mod kamino;
pub mod config;
pub mod state;

use pinocchio::program_error::ProgramError;
use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

use crate::{
    enums::{IntegrationConfig, IntegrationState}, 
    instructions::InitializeIntegrationArgs, 
    integrations::utilization_market::kamino::initialize::process_initialize_kamino, 
    processor::InitializeIntegrationAccounts, state::Controller
};

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, Default, PartialEq, ShankType)]
pub enum UtilizationMarket {
    #[default]
    Kamino
}

pub fn process_initialize_utilization_market(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
    market: UtilizationMarket,
    controller: &Controller
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    let (integration_config, integration_state) = match market {
        UtilizationMarket::Kamino => process_initialize_kamino(controller, outer_ctx, outer_args)?
    };

    Ok((integration_config, integration_state))
}