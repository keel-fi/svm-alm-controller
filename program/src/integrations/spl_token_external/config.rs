use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct SplTokenExternalConfig {
    pub program: Pubkey,
    pub mint: Pubkey,
    pub recipient: Pubkey,
    pub token_account: Pubkey,
    pub _padding: [u8; 160],
}
