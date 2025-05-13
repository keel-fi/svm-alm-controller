use super::{
    discriminator::{AccountDiscriminators, Discriminator},
    nova_account::NovaAccount,
};
use crate::{
    constants::CONTROLLER_SEED,
    enums::ControllerStatus,
    events::SvmAlmControllerEvent,
    processor::shared::{create_pda_account, emit_cpi},
};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
};
use pinocchio_token::instructions::Transfer;
use shank::ShankAccount;
use solana_program::pubkey::Pubkey as SolanaPubkey;

#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Controller {
    pub id: u16,
    pub bump: u8,
    pub status: ControllerStatus,
}

impl Discriminator for Controller {
    const DISCRIMINATOR: u8 = AccountDiscriminators::ControllerDiscriminator as u8;
}

impl NovaAccount for Controller {
    const LEN: usize = 4;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        Self::derive_pda_bytes(self.id)
    }
}

impl Controller {
    pub fn derive_pda_bytes(id: u16) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) = SolanaPubkey::find_program_address(
            &[CONTROLLER_SEED, id.to_le_bytes().as_ref()],
            &SolanaPubkey::from(crate::ID),
        );
        Ok((pda.to_bytes(), bump))
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

        // Create and serialize the controller
        let controller = Controller {
            id,
            bump: bump,
            status,
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
        status: Option<ControllerStatus>,
    ) -> Result<(), ProgramError> {
        // Update the status, if one is provided
        if let Some(status) = status {
            self.status = status;
        }
        // Commit the account on-chain
        self.save(account_info)?;

        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.status == ControllerStatus::Active
    }

    pub fn emit_event(
        &self,
        controller_info: &AccountInfo,
        event: SvmAlmControllerEvent,
    ) -> Result<(), ProgramError> {
        // Emit the Event to record the update
        emit_cpi(
            controller_info,
            [
                Seed::from(CONTROLLER_SEED),
                Seed::from(&self.id.to_le_bytes()),
                Seed::from(&[self.bump]),
            ],
            &self.id.to_le_bytes(),
            event,
        )?;
        Ok(())
    }

    pub fn transfer_tokens(
        &self,
        controller: &AccountInfo,
        vault: &AccountInfo,
        recipient_token_account: &AccountInfo,
        amount: u64,
    ) -> Result<(), ProgramError> {
        Transfer {
            from: vault,
            to: recipient_token_account,
            authority: controller,
            amount,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_SEED),
            Seed::from(&self.id.to_le_bytes()),
            Seed::from(&[self.bump]),
        ])])?;
        Ok(())
    }
}

// impl AccountDeserialize for Controller {}
