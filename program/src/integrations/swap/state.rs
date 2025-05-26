use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct AtomicSwapState {
    pub last_balance_a: u64,
    pub last_balance_b: u64,
    pub swap_started: bool,
    pub _padding: [u8; 31],
}
