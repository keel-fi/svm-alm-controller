use borsh::BorshDeserialize;
use pinocchio::{msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_log::log;

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

    const DISCRIMINATOR: [u8;8] = [159,131,58,170,193,84,128,182];     //9f 83 3a aa c1 54 80 b6

    pub fn deserialize(
        data: &[u8]
    ) -> Result<Self, ProgramError> {
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

    const DISCRIMINATOR: [u8;8] = [105,115,174,34,95,233,138,252];     //69 73 ae 22 5f e9 8a fc

    pub fn deserialize(
        data: &[u8]
    ) -> Result<Self, ProgramError> {
        if data[0..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Self::try_from_slice(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }
}

