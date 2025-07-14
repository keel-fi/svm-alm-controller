use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct CctpBridgeConfig {
    /// CCTP program for transferring tokens
    pub cctp_token_messenger_minter: Pubkey,
    /// CCTP program for sending messages
    pub cctp_message_transmitter: Pubkey,
    /// Mint of the token to be transferred
    pub mint: Pubkey,
    /// Destination of the token
    pub destination_address: Pubkey,
    /// Destination network of the token (i.e. Ethereum)
    pub destination_domain: u32,
    pub _padding: [u8; 92],
}
