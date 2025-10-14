use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct KaminoState {
    /// value of the liquidity deposited
    pub last_liquidity_value: u64,
    /// the lp amount (minted with push and burned with pull, called collateral in KLEND program)
    pub last_lp_amount: u64,
    /// padding
    pub _padding: [u8; 32],
}