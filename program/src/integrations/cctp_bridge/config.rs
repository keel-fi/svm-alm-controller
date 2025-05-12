use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct CctpBridgeConfig {
    pub cctp_token_messenger_minter: Pubkey,
    pub cctp_message_transmitter: Pubkey,
    pub mint: Pubkey,
    pub destination_address: Pubkey,
    pub destination_domain: u32,
    pub _padding: [u8; 60],
}
