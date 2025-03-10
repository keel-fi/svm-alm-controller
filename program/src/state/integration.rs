extern crate alloc;
use alloc::vec::Vec;
use shank::ShankAccount;
use crate::{
    acc_info_as_str,
    constants::INTEGRATION_SEED, 
    enums::{IntegrationConfig, IntegrationState, IntegrationStatus}, 
    processor::shared::create_pda_account
};
use super::discriminator::{AccountDiscriminators, Discriminator};
use solana_program::pubkey::Pubkey as SolanaPubkey;
use pinocchio::{
    account_info::AccountInfo, instruction::Seed, log, msg, program_error::ProgramError, pubkey::Pubkey, sysvars::{rent::Rent, Sysvar}
};
use pinocchio_log::log;
use borsh::{BorshDeserialize, BorshSerialize};


#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize,)]
#[repr(C)]
pub struct Integration {
    pub controller: Pubkey,
    pub description: [u8;32],
    pub hash: [u8;32],
    pub lookup_table: Pubkey,
    pub status: IntegrationStatus,
    pub config: IntegrationConfig,
    pub state: IntegrationState,

    // TODO: Rate Limiting
    // TODO: Track and freeze program upgrades
}


impl Discriminator for Integration {
    const DISCRIMINATOR: u8 = AccountDiscriminators::IntegrationDiscriminator as u8;
}

impl Integration {

    pub const LEN: usize = 4*32 + 1 + 193 + 33;

    pub fn verify_pda(
        &self,
        acc_info: &AccountInfo,
    ) -> Result<(), ProgramError> {
        let (pda, _bump) = Self::derive_pda(self.controller, self.hash)?;
        if acc_info.key().ne(&pda) {
            log!("PDA Mismatch for {}", acc_info_as_str!(acc_info));
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }

    pub fn derive_pda(
        controller: Pubkey,
        hash: [u8;32]
    ) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) = SolanaPubkey::find_program_address(
            &[INTEGRATION_SEED, controller.as_ref(), hash.as_ref()],
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
    ) -> Result<(), ProgramError> {
        if self.controller.ne(controller) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    pub fn load_and_check(
        account_info: &AccountInfo,
        controller: &Pubkey,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        // Check PDA
        
        let integration= Self::deserialize(&account_info.try_borrow_data()?).unwrap();
        integration.check_data(controller)?;
        integration.verify_pda(account_info)?;
        Ok(integration)
    }

    pub fn load_and_check_mut(
        account_info: &AccountInfo,
        controller: &Pubkey,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        let integration = Self::deserialize(&account_info.try_borrow_mut_data()?).unwrap();
        integration.check_data(controller)?;
        integration.verify_pda(account_info)?;
        Ok(integration)
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
        status: IntegrationStatus,
        config: IntegrationConfig,
        state: IntegrationState,
        description: [u8;32],
        lookup_table: Pubkey,
    ) -> Result<Self, ProgramError> {
        
        // Derive the hash for this config
        let hash = config.hash();

        // Derive the PDA
        let (pda, bump) = Self::derive_pda(controller, hash)?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds.into()); // PDA was invalid
        }
        
        // Create and serialize the controller
        let integration = Integration {
            controller,
            hash,
            status,
            lookup_table,
            description,
            config,
            state
        };

        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(INTEGRATION_SEED),
            Seed::from(&controller),
            Seed::from(&hash),
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
        integration.save(account_info)?;

        Ok(integration)
    }

    pub fn update_and_save(
        &mut self,
        account_info: &AccountInfo,
        status: Option<IntegrationStatus>,
        lookup_table: Option<Pubkey>
    ) -> Result<(), ProgramError> {
        
        if let Some(status) = status {
            self.status = status;
        }
        if let Some(lookup_table) = lookup_table {
            self.lookup_table = lookup_table;
        }
     
        // TODO: Add these 

        // Commit the account on-chain
        self.save(account_info)?;

        Ok(())
    }

    
}

