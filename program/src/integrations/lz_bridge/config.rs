use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct LzBridgeConfig {
    pub program: Pubkey,
    pub mint: Pubkey,
    pub oft_store: Pubkey,
    pub peer_config: Pubkey,
    pub token_escrow: Pubkey,
    pub destination_address: Pubkey,
    pub destination_eid: u32,
    pub _padding: [u8; 28],
}
