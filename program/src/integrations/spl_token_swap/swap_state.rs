use borsh::BorshDeserialize;
use pinocchio::pubkey::Pubkey;

#[derive(Debug, Default, PartialEq, BorshDeserialize)]
pub struct SwapV1Subset {
    /// Initialized state.
    pub is_initialized: bool,
    /// Bump seed used in program address.
    /// The program address is created deterministically with the bump seed,
    /// swap program id, and swap account pubkey.  This program address has
    /// authority over the swap's token A account, token B account, and pool
    /// token mint.
    pub bump_seed: u8,
    /// Program ID of the tokens being exchanged.
    pub token_program_id: Pubkey,
    /// Token A
    pub token_a: Pubkey,
    /// Token B
    pub token_b: Pubkey,
    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub pool_mint: Pubkey,

    /// Mint information for token A
    pub token_a_mint: Pubkey,
    /// Mint information for token B
    pub token_b_mint: Pubkey,

    /// Pool token account to receive trading and / or withdrawal fees
    pub pool_fee_account: Pubkey,

}

pub const LEN_SWAP_V1_SUBSET: usize = 7*32 + 1 + 1;
