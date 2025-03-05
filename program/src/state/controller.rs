extern crate alloc;
use alloc::vec::Vec;
use shank::ShankAccount;
use crate::{acc_info_as_str, constants::CONTROLLER_SEED, enums::ControllerStatus};
use super::discriminator::{AccountSerialize, AccountDiscriminators, Discriminator};
use solana_program::pubkey::Pubkey as SolanaPubkey;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_log::log;


#[derive(Clone, Debug, PartialEq, ShankAccount)]
#[repr(C)]
pub struct Controller {
    pub id: u16,
    pub bump: u8,
    pub status: u8,
}

impl Discriminator for Controller {
    const DISCRIMINATOR: u8 = AccountDiscriminators::ControllerDiscriminator as u8;
}

impl AccountSerialize for Controller {
    fn to_bytes_inner(&self) -> Vec<u8> {
        let mut data = Vec::new();
        // ID encoding
        data.extend_from_slice(self.id.to_le_bytes().as_ref());
        // Bump encoding
        data.extend_from_slice(self.bump.to_le_bytes().as_ref());
        // Status encoding
        data.extend_from_slice(self.status.to_le_bytes().as_ref());
        data
    }
}

impl Controller {
    pub fn verify_pda(
        &self,
        acc_info: &AccountInfo,
        program_id: &Pubkey,
    ) -> Result<(), ProgramError> {
        let (controller_pda, _controller_bump) = SolanaPubkey::find_program_address(
            &[CONTROLLER_SEED, self.id.to_le_bytes().as_ref()],
            &SolanaPubkey::from(*program_id),
        );
        if acc_info.key().ne(&controller_pda.to_bytes()) {
            log!("PDA Mismatch for {}", acc_info_as_str!(acc_info));
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }

    pub fn try_from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        // Check discriminator
        if data[0] != Self::DISCRIMINATOR {
            msg!("Invalid Credential Data");
            return Err(ProgramError::InvalidAccountData);
        }
        // Start offset after Discriminator
        let mut offset: usize = 1;

        let id: u16 = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as u16;
        offset += 2;
        let bump: u8 = u8::from_le_bytes(data[offset..offset + 1].try_into().unwrap()) as u8;
        offset += 1;
        let status: u8 = u8::from_le_bytes(data[offset..offset + 1].try_into().unwrap()) as u8;
        ControllerStatus::try_from(status).map_err(|_| ProgramError::InvalidAccountData )?;
        Ok(Self { id, bump, status })
    }
}
