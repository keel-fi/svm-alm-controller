use super::{
    discriminator::{AccountDiscriminators, Discriminator},
    nova_account::NovaAccount,
};
use crate::{
    constants::{CONTROLLER_AUTHORITY_SEED, CONTROLLER_SEED},
    enums::ControllerStatus,
    events::SvmAlmControllerEvent,
    processor::shared::{create_pda_account, emit_cpi},
};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
    sysvars::{rent::Rent, Sysvar},
};
use pinocchio_token::instructions::Transfer;
use shank::ShankAccount;

#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Controller {
    pub id: u16,
    pub bump: u8,
    pub status: ControllerStatus,
    pub authority: Pubkey,
    pub authority_bump: u8,
    pub _padding: [u8; 128],
}

impl Discriminator for Controller {
    const DISCRIMINATOR: u8 = AccountDiscriminators::ControllerDiscriminator as u8;
}

impl NovaAccount for Controller {
    // id + bump + status + authority + authority_bump + padding
    const LEN: usize = 2 + 1 + 1 + 32 + 1 + 128;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        Self::derive_pda_bytes(self.id)
    }
}

impl Controller {
    pub fn derive_pda_bytes(id: u16) -> Result<(Pubkey, u8), ProgramError> {
        try_find_program_address(
            &[CONTROLLER_SEED, id.to_le_bytes().as_ref()],
            &crate::ID,
        ).ok_or(ProgramError::InvalidSeeds)
    }

    pub fn derive_authority(controller: &Pubkey) -> Result<(Pubkey, u8), ProgramError> {
        try_find_program_address(
            &[CONTROLLER_AUTHORITY_SEED, controller.as_ref()],
            &crate::ID,
        )
        .ok_or(ProgramError::InvalidSeeds)
    }

    pub fn load_and_check(account_info: &AccountInfo) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        let controller: Self = NovaAccount::deserialize(&account_info.try_borrow_data()?).unwrap();
        controller.verify_pda(account_info)?;
        Ok(controller)
    }

    pub fn load_and_check_mut(account_info: &AccountInfo) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        let controller: Self =
            NovaAccount::deserialize(&account_info.try_borrow_mut_data()?).unwrap();
        controller.verify_pda(account_info)?;
        Ok(controller)
    }

    pub fn init_account(
        account_info: &AccountInfo,
        authority_info: &AccountInfo,
        payer_info: &AccountInfo,
        id: u16,
        status: ControllerStatus,
    ) -> Result<Self, ProgramError> {
        // Derive the PDA
        let controller_id = id.to_le_bytes();
        let (pda, bump) = Self::derive_pda_bytes(id)?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds.into()); // PDA was invalid
        }

        // Derive authority PDA that has no SOL or data
        let (controller_authority, controller_authority_bump) =
            Controller::derive_authority(account_info.key())?;

        if authority_info.key().ne(&controller_authority) {
            // Authority PDA was invalid
            return Err(ProgramError::InvalidSeeds.into());
        }

        // Create and serialize the controller
        let controller = Controller {
            id,
            bump: bump,
            status,
            authority: controller_authority,
            authority_bump: controller_authority_bump,
            _padding: [0; 128],
        };

        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller_id),
            Seed::from(&bump_seed),
        ];
        create_pda_account(
            payer_info,
            &rent,
            1 + Self::LEN,
            &crate::ID,
            account_info,
            signer_seeds,
        )?;

        // Commit the account on-chain
        controller.save(account_info)?;

        Ok(controller)
    }

    pub fn update_and_save(
        &mut self,
        account_info: &AccountInfo,
        status: ControllerStatus
    ) -> Result<(), ProgramError> {

        // No change will take place
        if self.status == status {
            return Err(ProgramError::InvalidArgument.into());
        }

        // Update the status, 
        self.status = status;
    
        // Commit the account on-chain
        self.save(account_info)?;

        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.status == ControllerStatus::Active
    }

    pub fn emit_event(
        &self,
        authority_info: &AccountInfo,
        controller: &Pubkey,
        event: SvmAlmControllerEvent,
    ) -> Result<(), ProgramError> {
        // Emit the Event to record the update
        emit_cpi(
            authority_info,
            [
                Seed::from(CONTROLLER_AUTHORITY_SEED),
                Seed::from(controller),
                Seed::from(&[self.authority_bump]),
            ],
            &self.id.to_le_bytes(),
            event,
        )?;
        Ok(())
    }

    pub fn transfer_tokens(
        &self,
        controller: &AccountInfo,
        controller_authority: &AccountInfo,
        vault: &AccountInfo,
        recipient_token_account: &AccountInfo,
        amount: u64,
    ) -> Result<(), ProgramError> {
        Transfer {
            from: vault,
            to: recipient_token_account,
            authority: controller_authority,
            amount,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(controller.key()),
            Seed::from(&[self.authority_bump]),
        ])])?;
        Ok(())
    }
}
