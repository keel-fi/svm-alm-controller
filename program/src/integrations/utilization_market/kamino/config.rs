use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct KaminoConfig {
    /// The Kamino market
    pub market: Pubkey,
    /// the Kamino reserve, linked to token_mint
    pub reserve: Pubkey,
    /// the reserve farm collateral (can be Pubkey::default())
    pub reserve_farm_collateral: Pubkey,
    /// the reserve farm debt (can be Pubkey::default())
    pub reserve_farm_debt: Pubkey,
    /// The reserve liquidity mint. This is the mint that is deposited (lent) into the reserve.
    pub reserve_liquidity_mint: Pubkey,
    /// The obligation, KaminoConfigs can share obligations
    pub obligation: Pubkey,
    /// obligation_id: helper for the UI?
    pub obligation_id: u8,
    /// padding
    pub _padding: [u8; 30]
}