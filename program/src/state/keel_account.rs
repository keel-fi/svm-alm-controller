extern crate alloc;

use crate::{acc_info_as_str, error::SvmAlmControllerErrors};
use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_log::log;

use super::Discriminator;

pub trait KeelAccount: Discriminator + BorshDeserialize + BorshSerialize {
    /// The size in bytes for the discriminator
    const DISCRIMINATOR_SIZE: usize = 1;
    /// The size in bytes for the account data (sans discriminator)
    const LEN: usize;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError>;

    fn verify_pda(&self, acc_info: &AccountInfo) -> Result<(), ProgramError> {
        let (pda, _bump) = self.derive_pda()?;
        if acc_info.key().ne(&pda) {
            log!("PDA mismatch for {}", acc_info_as_str!(acc_info));
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }
        Ok(())
    }

    /// Save the DISCRIMINATOR and data to an account
    fn save(&self, account_info: &AccountInfo) -> Result<(), ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        let mut serialized = Vec::with_capacity(Self::DISCRIMINATOR_SIZE + Self::LEN);
        serialized.push(Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Ensure account has enough space
        if account_info.data_len() != serialized.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        // Copy serialized data to account
        let mut data = account_info.try_borrow_mut_data()?;
        data[..serialized.len()].copy_from_slice(&serialized);
        Ok(())
    }

    fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        // Check discriminator
        if data[0] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        // Use Borsh deserialization
        Self::try_from_slice(&data[Self::DISCRIMINATOR_SIZE..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}
