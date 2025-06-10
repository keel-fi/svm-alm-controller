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
    /// The max amount of slippage from the oracle's price accepted.
    pub max_slippage_bps: u16,
    /// Max allowed staleness of oracle's last_update_slot from clock slot.
    pub max_staleness: u64,
    /// Input token mint's decimals
    pub input_mint_decimals: u8,
    /// Ouput token mint's decimals
    pub output_mint_decimals: u8,
    /// Expiry time of swap
    pub expiry_timestamp: i64,
    pub padding: [u8; 108],
}
