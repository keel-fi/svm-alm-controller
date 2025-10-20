use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct DriftState {
    /// The deposit amount (scaled balance * cumulative deposit interest)
    pub balance: u64,
    /// Padding
    pub _padding: [u8; 32],
}
