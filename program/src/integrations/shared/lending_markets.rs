use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct LendingState {
    /// The balance of tokens that the Controller has a claim on.
    /// This includes the deposit amount plus any interest earned.
    pub balance: u64,
    /// Padding
    pub _padding: [u8; 40],
}
