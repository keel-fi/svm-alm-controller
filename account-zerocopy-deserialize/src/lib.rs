use bytemuck::Pod;
use pinocchio::program_error::ProgramError;

/// Trait to deserialize accounts that are bytemuck compatible with a
/// sized discriminator. This will work for anchor generated 8-byte
/// discriminators as well as custom sized discriminators.
pub trait AccountZerocopyDeserialize<const N: usize>: Sized + Pod {
    const DISCRIMINATOR: [u8; N];

    /// Deserialize account into immutable struct.
    fn try_from_slice(data: &[u8]) -> Result<&Self, ProgramError> {
        let disc_len = Self::DISCRIMINATOR.len();
        if data
            .get(..disc_len)
            .ok_or(ProgramError::InvalidAccountData)?
            .ne(&Self::DISCRIMINATOR)
        {
            return Err(ProgramError::InvalidAccountData);
        }
        bytemuck::try_from_bytes(&data[disc_len..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    fn try_from_slice_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
        let disc_len = Self::DISCRIMINATOR.len();
        if data
            .get(..disc_len)
            .ok_or(ProgramError::InvalidAccountData)?
            .ne(&Self::DISCRIMINATOR)
        {
            return Err(ProgramError::InvalidAccountData);
        }
        bytemuck::try_from_bytes_mut(&mut data[disc_len..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}
