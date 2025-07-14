use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

/// Configuration for the Controller to LP into a specific market of the SPL Token Swap program.
#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct SplTokenSwapConfig {
    /// SPL Token swap program
    pub program: Pubkey,
    /// Swap market state for the given mints
    pub swap: Pubkey,
    /// Token mint a
    pub mint_a: Pubkey,
    /// Token mint b
    pub mint_b: Pubkey,
    /// Token min of the LP token for liquidity positions of the market
    pub lp_mint: Pubkey,
    /// LP TokenAccount owned by the Controller
    pub lp_token_account: Pubkey,
    pub _padding: [u8; 32],
}
