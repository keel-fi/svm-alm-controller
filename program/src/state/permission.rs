extern crate alloc;
use alloc::vec::Vec;
use shank::ShankAccount;
use crate::{acc_info_as_str, constants::PERMISSION_SEED, enums::PermissionStatus, processor::shared::create_pda_account};
use super::discriminator::{AccountDiscriminators, Discriminator};
use solana_program::pubkey::Pubkey as SolanaPubkey;
use pinocchio::{
    account_info::AccountInfo, instruction::Seed, program_error::ProgramError, pubkey::Pubkey, sysvars::{rent::Rent, Sysvar}
};
use pinocchio_log::log;
use borsh::{BorshDeserialize, BorshSerialize};


#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize,)]
#[repr(C)]
pub struct Permission {
    pub controller: Pubkey,
    pub authority: Pubkey,
    pub status: PermissionStatus,
    pub can_manage_permissions: bool,
    pub can_invoke_external_transfer: bool,
    pub can_execute_swap: bool,
    pub can_reallocate: bool,
    pub can_freeze: bool,
    pub can_unfreeze: bool,
    pub can_manage_integrations: bool,
}

impl Discriminator for Permission {
    const DISCRIMINATOR: u8 = AccountDiscriminators::PermissionDiscriminator as u8;
}

impl Permission {

    pub const LEN: usize = 65 +7;

    pub fn verify_pda(
        &self,
        acc_info: &AccountInfo,
    ) -> Result<(), ProgramError> {
        let (pda, _bump) = Self::derive_pda(self.controller, self.authority)?;
        if acc_info.key().ne(&pda) {
            log!("PDA Mismatch for {}", acc_info_as_str!(acc_info));
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }

    pub fn derive_pda(
        controller: Pubkey,
        authority: Pubkey
    ) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) = SolanaPubkey::find_program_address(
            &[PERMISSION_SEED, controller.as_ref(), authority.as_ref()],
            &SolanaPubkey::from(crate::ID),
        );
        Ok((pda.to_bytes(), bump))
    }

    fn deserialize(
        data: &[u8]
    ) -> Result<Self, ProgramError> {
        // Check discriminator
        if data[0] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        // Use Borsh deserialization
        Self::try_from_slice(&data[1..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn check_data(
        &self,
        controller: &Pubkey,
        authority: &Pubkey
    ) -> Result<(), ProgramError> {
        if self.authority.ne(authority) || self.controller.ne(controller) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    pub fn load_and_check(
        account_info: &AccountInfo,
        controller: &Pubkey,
        authority: &Pubkey
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        // Check PDA
        
        let permission= Self::deserialize(&account_info.try_borrow_data()?).unwrap();
        permission.check_data(controller, authority)?;
        permission.verify_pda(account_info)?;
        Ok(permission)
    }

    pub fn load_and_check_mut(
        account_info: &AccountInfo,
        controller: &Pubkey,
        authority: &Pubkey
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        let permission = Self::deserialize(&account_info.try_borrow_mut_data()?).unwrap();
        permission.check_data(controller, authority)?;
        permission.verify_pda(account_info)?;
        Ok(permission)
    }

    pub fn save(&self, account_info: &AccountInfo) -> Result<(), ProgramError> {
        // Ensure account owner is the program
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        
        let mut serialized = Vec::with_capacity(1 + Self::LEN);
        serialized.push(Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        // Ensure account has enough space
        if account_info.data_len() < serialized.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        // Copy serialized data to account
        let mut data = account_info.try_borrow_mut_data()?;
        data[..serialized.len()].copy_from_slice(&serialized);
        
        Ok(())
    }

    pub fn init_account(
        account_info: &AccountInfo,
        payer_info: &AccountInfo,
        controller: Pubkey,
        authority: Pubkey,
        status: PermissionStatus,
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_execute_swap: bool,
        can_reallocate: bool,
        can_freeze: bool,
        can_unfreeze: bool,
        can_manage_integrations: bool,
    ) -> Result<Self, ProgramError> {
        
        // Derive the PDA
        let (pda, bump) = Self::derive_pda(controller, authority)?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds.into()); // PDA was invalid
        }

        // Create and serialize the controller
        let permission = Permission {
            controller,
            authority,
            status,
            can_manage_permissions,
            can_invoke_external_transfer,
            can_execute_swap,
            can_reallocate,
            can_freeze,
            can_unfreeze,
            can_manage_integrations,
        };

        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(PERMISSION_SEED),
            Seed::from(&controller),
            Seed::from(&authority),
            Seed::from(&bump_seed)
        ];
        create_pda_account(
            payer_info,
            &rent,
            1 + Self::LEN, 
            &crate::ID,
            account_info, 
            signer_seeds
        )?;
        
        // Commit the account on-chain
        permission.save(account_info)?;

        Ok(permission)
    }

    pub fn update_and_save(
        &mut self,
        account_info: &AccountInfo,
        status: Option<PermissionStatus>,
        can_manage_permissions: Option<bool>,
        can_invoke_external_transfer: Option<bool>,
        can_execute_swap: Option<bool>,
        can_reallocate: Option<bool>,
        can_freeze: Option<bool>,
        can_unfreeze: Option<bool>,
        can_manage_integrations: Option<bool>,
    ) -> Result<(), ProgramError> {
        
        if let Some(status) = status {
            self.status = status;
        }
        if let Some(can_manage_permissions) = can_manage_permissions {
            self.can_manage_permissions = can_manage_permissions;
        }
        if let Some(can_invoke_external_transfer) = can_invoke_external_transfer {
            self.can_invoke_external_transfer = can_invoke_external_transfer;
        }
        if let Some(can_execute_swap) = can_execute_swap {
            self.can_execute_swap = can_execute_swap;
        }
        if let Some(can_reallocate) = can_reallocate {
            self.can_reallocate = can_reallocate;
        }
        if let Some(can_freeze) = can_freeze {
            self.can_freeze = can_freeze;
        }
        if let Some(can_unfreeze) = can_unfreeze {
            self.can_unfreeze = can_unfreeze;
        }
        if let Some(can_manage_integrations) = can_manage_integrations {
            self.can_manage_integrations = can_manage_integrations;
        }
        // Commit the account on-chain
        self.save(account_info)?;

        Ok(())
    }

    pub fn can_manage_permissions(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_manage_permissions
    }

    pub fn can_manage_integrations(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_manage_integrations
    }

    pub fn can_reallocate(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_reallocate
    }
    
    
}

