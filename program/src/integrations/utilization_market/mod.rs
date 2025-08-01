pub mod kamino;

use pinocchio::{program_error::ProgramError, pubkey::Pubkey};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio_pubkey::pubkey;
use shank::ShankType;

use crate::{
    enums::{IntegrationConfig, IntegrationState}, 
    instructions::InitializeIntegrationArgs, 
    integrations::utilization_market::kamino::initialize::process_initialize_kamino, 
    processor::InitializeIntegrationAccounts, state::Controller
};

pub const KAMINO_LEND_PROGRAM_ID: Pubkey = pubkey!("GzFgdRJXmawPhGeBsyRCDLx4jAKPsvbUqoqitzppkzkW");
pub const KAMINO_FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
pub const LOOKUP_TABLE_PROGRAM_ID: Pubkey = pubkey!("AddressLookupTab1e1111111111111111111111111");

pub trait BorrowLendUtilizationMarket {

}

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