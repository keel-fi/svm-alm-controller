extern crate alloc;
use alloc::vec::Vec;
use shank::ShankAccount;
use crate::{acc_info_as_str, constants::INTEGRATION_SEED, enums::IntegrationStatus};
use super::discriminator::{AccountSerialize, AccountDiscriminators, Discriminator};
use solana_program::pubkey::Pubkey as SolanaPubkey;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_log::log;


#[derive(Clone, Debug, PartialEq, ShankAccount)]
#[repr(C)]
pub struct Integration {
    pub controller: Pubkey,
    pub program: Pubkey,
    pub status: u8,
}

impl Discriminator for Integration {
    const DISCRIMINATOR: u8 = AccountDiscriminators::IntegrationDiscriminator as u8;
}

impl AccountSerialize for Integration {
    fn to_bytes_inner(&self) -> Vec<u8> {
        let mut data = Vec::new();
        // Authority encoding
        data.extend_from_slice(self.controller.as_ref());
        // Program encoding
        data.extend_from_slice(self.program.as_ref());
        // Status encoding
        data.extend_from_slice(self.status.to_le_bytes().as_ref());
        data
    }
}

impl Integration {
    pub fn verify_pda(
        &self,
        acc_info: &AccountInfo,
        program_id: &Pubkey,
    ) -> Result<(), ProgramError> {
        let (integration_pda, _integration_bump) = SolanaPubkey::find_program_address(
            &[INTEGRATION_SEED, self.controller.as_ref(), self.program.as_ref()],
            &SolanaPubkey::from(*program_id),
        );
        if acc_info.key().ne(&integration_pda.to_bytes()) {
            log!("PDA Mismatch for {}", acc_info_as_str!(acc_info));
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }

    pub fn validate_controller(&self, controller: &Pubkey) -> Result<(), ProgramError> {
        if self.controller.ne(controller) {
            log!("Controller Mismatch");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    pub fn validate_program(&self, program: &Pubkey) -> Result<(), ProgramError> {
        if self.program.ne(program) {
            log!("Program Mismatch");
            return Err(ProgramError::InvalidAccountData);
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
        // Controller pubkey
        let controller: Pubkey = data[offset..offset + 32].try_into().unwrap();
        offset += 32;
        // Program pubkey
        let program: Pubkey = data[offset..offset + 32].try_into().unwrap();
        offset += 32;
        // Status
        let status: u8 = u8::from_le_bytes(data[offset..offset + 1].try_into().unwrap()) as u8;
        IntegrationStatus::try_from(status).map_err(|_| ProgramError::InvalidAccountData )?;
        Ok(Self { controller, program, status })
    }
    
}
