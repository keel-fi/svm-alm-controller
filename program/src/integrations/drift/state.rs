use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct DriftState {
    /// Value of the liquidity deposited (in quote asset precision)
    pub last_liquidity_value: u64,
    /// The deposit amount (scaled balance * cumulative deposit interest)
    pub last_deposit_amount: u64,
    /// Padding
    pub _padding: [u8; 32],
}
