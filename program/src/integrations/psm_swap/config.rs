use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct PsmSwapConfig {
    pub psm_token: Pubkey,
    pub psm_pool: Pubkey,
    pub mint: Pubkey,
    pub _padding: [u8; 128]
}