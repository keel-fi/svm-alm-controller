use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

use crate::integrations::utilization_market::UtilizationMarket;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, Default, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationType {
    #[default]
    SplTokenExternal,
    CctpBridge,
    LzBridge,
    AtomicSwap,
    UtilizationMarket(UtilizationMarket)
}
