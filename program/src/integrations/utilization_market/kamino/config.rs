use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct KaminoConfig {
    /// The Kamino market
    pub market: Pubkey,
    /// the Kamino reserve, linked to token_mint
    pub reserve: Pubkey,
    /// the reserve farm (can be Pubkey::default())
    pub reserve_farm: Pubkey,
    /// The mint that's pushed/pulled into/from market
    pub token_mint: Pubkey,
    /// The obligation, KaminoConfigs can share obligations
    pub obligation: Pubkey,
    /// obligation_id: helper for the UI?
    pub obligation_id: u8,
}