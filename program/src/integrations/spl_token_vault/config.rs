use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct SplTokenVaultConfig {
    pub program: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub _padding: [u8; 96]
}