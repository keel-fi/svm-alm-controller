use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

use crate::integrations::utilization_market::kamino::config::KaminoConfig;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum UtilizationMarketConfig {
    KaminoConfig(KaminoConfig),
}