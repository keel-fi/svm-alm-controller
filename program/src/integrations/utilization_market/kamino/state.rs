use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct KaminoState {
    /// value of liquidity deposited
    pub deposited_liquidity_value: u64,
    /// the collateral amount (minted with push and burned with pull)
    pub last_collateral_amount: u64,
    /// padding
    pub _padding: [u8; 31],
}