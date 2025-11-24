use borsh::BorshDeserialize;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

use crate::constants::anchor_discriminator;

#[derive(Debug, Default, PartialEq, BorshDeserialize)]
pub struct LocalToken {
    pub custody: Pubkey,
    pub mint: Pubkey,
    pub burn_limit_per_message: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub amount_sent: u128,
    pub amount_received: u128,
    pub bump: u8,
    pub custody_bump: u8,
}

impl LocalToken {
    const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "LocalToken");

    pub fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        if data[0..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Self::try_from_slice(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }
}

#[derive(Debug, Default, PartialEq, BorshDeserialize)]
pub struct RemoteTokenMessenger {
    pub domain: u32,
    pub token_messenger: Pubkey,
}

impl RemoteTokenMessenger {
    const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "RemoteTokenMessenger");

    pub fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        if data[0..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Self::try_from_slice(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }
}
