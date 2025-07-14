use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

/// An Integration that allows for transferring tokens to wallets external to the Controller.
#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct SplTokenExternalConfig {
    /// Token program that controls the mint and thus the transferring of tokens
    pub program: Pubkey,
    /// Mint of the token that can be transferred by this Integration
    pub mint: Pubkey,
    /// The allowed recipient of the Controller's tokens
    pub recipient: Pubkey,
    /// TokenAccount to receive the tokens, which must be the Recipients ATA
    pub token_account: Pubkey,
    pub _padding: [u8; 96],
}
