use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

use crate::integrations::utilization_market::kamino::state::KaminoState;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum UtilizationMarketState {
    KaminoState(KaminoState),
}