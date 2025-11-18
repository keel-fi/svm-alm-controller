use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct PsmSwapState {
    /// `liquidity_supplied` tracks the liquidity deposited into Token (PSM account) vault
    pub liquidity_supplied: u64,
    pub _padding: [u8; 40]
}