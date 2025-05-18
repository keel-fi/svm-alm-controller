use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct AtomicSwapConfig {
    /// The token mint that is being used to make the swap
    pub input_token: Pubkey,
    /// The token being swapped for
    pub output_token: Pubkey,
    /// The Oracle account that is used for this pair
    pub oracle: Pubkey,
    /// The max amount of slippage from the oracle's price.
    pub max_slippage_bps: u16,
    pub padding: [u8; 94],
}
