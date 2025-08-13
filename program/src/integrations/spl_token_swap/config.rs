use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
};
use shank::ShankType;

use crate::constants::SPL_TOKEN_SWAP_LP_SEED;

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

impl SplTokenSwapConfig {
    /// Derive the SplTokenSwap LP Token Account address for a given Controller.
    pub fn derive_lp_token_account_pda(
        controller: &Pubkey,
        lp_mint: &Pubkey,
    ) -> Result<(Pubkey, u8), ProgramError> {
        try_find_program_address(&[SPL_TOKEN_SWAP_LP_SEED, controller, lp_mint], &crate::ID)
            .ok_or(ProgramError::InvalidSeeds)
    }
}
