use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct AtomicSwapState {
    // Amount of token a in reserve before swap.
    pub last_balance_a: u64,
    // Amount of token b in reserve before swap.
    pub last_balance_b: u64,
    // Amount of token a borrowed
    pub amount_borrowed: u64,
    pub _padding: [u8; 24],
}

impl AtomicSwapState {
    pub fn has_swap_started(&self) -> bool {
        self.amount_borrowed > 0
    }
}
