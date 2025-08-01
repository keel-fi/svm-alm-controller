use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct KaminoConfig {
    /// The Kamino market
    pub market: Pubkey,
    /// The mint that's pushed/pulled into/from market
    pub mint: Pubkey,
    /// The obligation, KaminoConfigs can share obligations
    pub obligation: Pubkey
}