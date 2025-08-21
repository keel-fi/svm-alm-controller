use solana_program::pubkey::Pubkey;

use crate::SPL_TOKEN_SWAP_LP_SEED;

/// Derive the LP Token Account address for a given SplTokenSwap integration.
pub fn derive_spl_token_swap_lp_pda(controller: &Pubkey, lp_mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            SPL_TOKEN_SWAP_LP_SEED,
            controller.as_ref(),
            lp_mint.as_ref(),
        ],
        &crate::SVM_ALM_CONTROLLER_ID,
    )
    .0
}
