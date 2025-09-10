use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

/// Configuration for sending Tokens via LayerZero's OFT standard to external chains.
#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct LzBridgeConfig {
    /// OFT program that is used for the Send transaction
    pub program: Pubkey,
    /// Mint of the token to be transferred cross-chain
    pub mint: Pubkey,
    /// OFT Store account
    pub oft_store: Pubkey,
    /// Peer (aka other network) OFT configuration
    pub peer_config: Pubkey,
    /// Escrow account of the OFT
    pub oft_token_escrow: Pubkey,
    /// Destination to receive the tokens
    pub destination_address: Pubkey,
    /// ID of the destination chain
    pub destination_eid: u32,
    pub _padding: [u8; 28],
}
