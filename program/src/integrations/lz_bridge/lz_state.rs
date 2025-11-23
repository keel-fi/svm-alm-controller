use borsh::BorshDeserialize;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

use crate::constants::anchor_discriminator;

pub const OFT_PEER_CONFIG_SEED: &[u8] = b"Peer";

#[derive(BorshDeserialize)]
pub struct OFTStore {
    pub oft_type: u8,
    pub ld2sd_rate: u64,
    pub token_mint: Pubkey,
    pub token_escrow: Pubkey,
    pub endpoint_program: Pubkey,
    pub bump: u8,
    pub tvl_ld: u64,
    pub admin: Pubkey,
    pub default_fee_bps: u16,
    pub paused: bool,
    // pub pauser: Option<Pubkey>,
    // pub unpauser: Option<Pubkey>,
}

// #[derive(BorshDeserialize, PartialEq, Eq)]
// pub enum OFTType {
//     Native,
//     Adapter,
// }

impl OFTStore {
    const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "OFTStore");
    const TRUNCATED_LEN: usize = 149;

    pub fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        if data[0..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Self::try_from_slice(&data[8..Self::TRUNCATED_LEN + 8])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

#[derive(BorshDeserialize)]
pub struct PeerConfig {
    pub peer_address: [u8; 32],
    // pub enforced_options: EnforcedOptions,
    // pub outbound_rate_limiter: Option<RateLimiter>,
    // pub inbound_rate_limiter: Option<RateLimiter>,
    // pub fee_bps: Option<u16>,
    // pub bump: u8,
}

impl PeerConfig {
    const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "PeerConfig");
    const TRUNCATED_LEN: usize = 32;

    pub fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        if data[0..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Self::try_from_slice(&data[8..Self::TRUNCATED_LEN + 8])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

// #[derive(BorshDeserialize)]
// pub struct RateLimiter {
//     pub capacity: u64,
//     pub tokens: u64,
//     pub refill_per_second: u64,
//     pub last_refill_time: u64,
// }
