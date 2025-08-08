use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct KaminoState {
    /// the lending amount
    pub assets: u64,
    /// the lent amount
    pub liabilities: u64,
    /// padding
    pub _padding: [u8; 31],
}