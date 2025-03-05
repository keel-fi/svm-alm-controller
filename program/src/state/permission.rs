extern crate alloc;
use alloc::vec::Vec;
use shank::ShankAccount;
use crate::{acc_info_as_str, constants::PERMISSION_SEED, enums::PermissionStatus};
use super::discriminator::{AccountSerialize, AccountDiscriminators, Discriminator};
use solana_program::pubkey::Pubkey as SolanaPubkey;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_log::log;


#[derive(Clone, Debug, PartialEq, ShankAccount)]
#[repr(C)]
pub struct Permission {
    pub controller: Pubkey,
    pub authority: Pubkey,
    pub status: u8,
}

impl Discriminator for Permission {
    const DISCRIMINATOR: u8 = AccountDiscriminators::PermissionDiscriminator as u8;
}

impl AccountSerialize for Permission {
    fn to_bytes_inner(&self) -> Vec<u8> {
        let mut data = Vec::new();
        // Authority encoding
        data.extend_from_slice(self.controller.as_ref());
        // Authority encoding
        data.extend_from_slice(self.authority.as_ref());
        // Status encoding
        data.extend_from_slice(self.status.to_le_bytes().as_ref());
        data
    }
}

impl Permission {
    pub fn verify_pda(
        &self,
        acc_info: &AccountInfo,
        program_id: &Pubkey,
    ) -> Result<(), ProgramError> {
        let (permission_pda, _permission_bump) = SolanaPubkey::find_program_address(
            &[PERMISSION_SEED, self.controller.as_ref(), self.authority.as_ref()],
            &SolanaPubkey::from(*program_id),
        );
        if acc_info.key().ne(&permission_pda.to_bytes()) {
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

    pub fn validate_authority(&self, authority: &Pubkey) -> Result<(), ProgramError> {
        if self.authority.ne(authority) {
            log!("Authority Mismatch");
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
        // Authority pubkey
        let authority: Pubkey = data[offset..offset + 32].try_into().unwrap();
        offset += 32;
        // Status
        let status: u8 = u8::from_le_bytes(data[offset..offset + 1].try_into().unwrap()) as u8;
        PermissionStatus::try_from(status).map_err(|_| ProgramError::InvalidAccountData )?;
        Ok(Self { controller, authority, status })
    }
    
}
