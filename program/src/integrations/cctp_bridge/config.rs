use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct CctpBridgeConfig {
    pub program: Pubkey,
    pub mint: Pubkey,
    pub destination_address: Pubkey,
    pub destination_domain: u32,
    pub _padding: [u8;92]
}

