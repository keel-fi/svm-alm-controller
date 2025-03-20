extern crate alloc;
use alloc::vec::Vec;
use pinocchio_token::state::TokenAccount;
use shank::ShankAccount;
use crate::{
    acc_info_as_str, 
    constants::RESERVE_SEED, 
    enums::ReserveStatus, 
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent}, 
    processor::shared::create_pda_account
};
use super::{discriminator::{AccountDiscriminators, Discriminator}, Controller};
use solana_program::{clock::SECONDS_PER_DAY, pubkey::Pubkey as SolanaPubkey};
use pinocchio::{
    account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::Pubkey, sysvars::{clock::Clock, rent::Rent, Sysvar}
};
use pinocchio_log::log;
use borsh::{BorshDeserialize, BorshSerialize};



#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize,)]
#[repr(C)]
pub struct Reserve {
    pub controller: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub status: ReserveStatus,
    pub rate_limit_slope: u64,
    pub rate_limit_max_outflow: u64,
    pub rate_limit_amount_last_update: u64, 
    pub last_balance: u64,
    pub last_refresh_timestamp: i64,
    pub last_refresh_slot: u64,
}


impl Discriminator for Reserve {
    const DISCRIMINATOR: u8 = AccountDiscriminators::ReserveDiscriminator as u8;
}

impl Reserve {

    pub const LEN: usize = 32*3 + 8*6 + 1;

    pub fn verify_pda(
        &self,
        acc_info: &AccountInfo,
    ) -> Result<(), ProgramError> {
        let (pda, _bump) = Self::derive_pda(self.controller, self.mint)?;
        if acc_info.key().ne(&pda) {
            log!("PDA Mismatch for {}", acc_info_as_str!(acc_info));
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }

    pub fn derive_pda(
        controller: Pubkey,
        mint: Pubkey
    ) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) = SolanaPubkey::find_program_address(
            &[RESERVE_SEED, controller.as_ref(), mint.as_ref()],
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
        
        let reserve= Self::deserialize(&account_info.try_borrow_data()?).unwrap();
        reserve.check_data(controller)?;
        reserve.verify_pda(account_info)?;
        Ok(reserve)
    }

    pub fn load_and_check_mut(
        account_info: &AccountInfo,
        controller: &Pubkey,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        let reserve = Self::deserialize(&account_info.try_borrow_mut_data()?).unwrap();
        reserve.check_data(controller)?;
        reserve.verify_pda(account_info)?;
        Ok(reserve)
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
        mint: Pubkey,
        vault: Pubkey,
        status: ReserveStatus,
        rate_limit_slope: u64,
        rate_limit_max_outflow: u64,
    ) -> Result<Self, ProgramError> {
        // Derive the PDA
        let (pda, bump) = Self::derive_pda(controller, mint)?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds.into()); // PDA was invalid
        }
        // Create and serialize the controller
        let clock = Clock::get()?;
        let reserve = Reserve {
            controller,
            mint,
            vault,
            status,
            rate_limit_slope,
            rate_limit_max_outflow,
            rate_limit_amount_last_update: rate_limit_max_outflow, // Starts at full amount
            last_balance: 0,
            last_refresh_timestamp: clock.unix_timestamp,
            last_refresh_slot: clock.slot,
        };
        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(RESERVE_SEED),
            Seed::from(&controller),
            Seed::from(&mint),
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
        reserve.save(account_info)?;
        Ok(reserve)
    }

    pub fn update(
        &mut self,
        status: Option<ReserveStatus>,
        rate_limit_slope: Option<u64>,
        rate_limit_max_outflow: Option<u64>,
    ) -> Result<(), ProgramError> {
        if let Some(status) = status {
            self.status = status;
        }
        if let Some(rate_limit_slope) = rate_limit_slope {
            self.rate_limit_slope = rate_limit_slope;
        }
        if let Some(rate_limit_max_outflow) = rate_limit_max_outflow {
            self.rate_limit_max_outflow = rate_limit_max_outflow;
        }
        Ok(())
    }

    pub fn refresh_rate_limit(
        &mut self,
        clock: Clock
    ) -> Result<(), ProgramError> {
        if self.rate_limit_max_outflow == u64::MAX || self.last_refresh_timestamp == clock.unix_timestamp {
            () // Do nothing
        } else {
            self.rate_limit_amount_last_update = self.rate_limit_amount_last_update.checked_add(
                (self.rate_limit_slope as u128 * clock.unix_timestamp.checked_sub(self.last_refresh_timestamp).unwrap() as u128 / SECONDS_PER_DAY as u128) as u64
            ).unwrap_or(self.rate_limit_max_outflow);
        }
        self.last_refresh_timestamp = clock.unix_timestamp;
        self.last_refresh_slot = clock.slot;
        Ok(())
    }

    pub fn update_for_inflow(
        &mut self,
        clock: Clock,
        inflow: u64,
    ) -> Result<(), ProgramError> {
        if !(self.last_refresh_timestamp != clock.unix_timestamp && self.last_refresh_slot == clock.slot) {
            msg!{"Rate limit must be refreshed before updating for flows"}
            return Err(ProgramError::InvalidArgument)
        }
        // Cap the rate_limit_amount_last_update at the rate_limit_max_outflow
        let v = self.rate_limit_amount_last_update.saturating_add(inflow);
        if v > self.rate_limit_max_outflow { // Cannot daily max outflow
            self.rate_limit_amount_last_update = self.rate_limit_max_outflow;
        } else {
            self.rate_limit_amount_last_update = v;
        }
        self.last_balance = self.last_balance.checked_add(inflow).unwrap();
        Ok(())
    }

    pub fn update_for_outflow(
        &mut self,
        clock: Clock,
        outflow: u64,
    ) -> Result<(), ProgramError> {
        if !(self.last_refresh_timestamp != clock.unix_timestamp && self.last_refresh_slot == clock.slot) {
            msg!{"Rate limit must be refreshed before updating for flows"}
            return Err(ProgramError::InvalidArgument)
        }
        self.rate_limit_amount_last_update = self.rate_limit_amount_last_update.checked_sub(outflow).unwrap();
        self.last_balance = self.last_balance.checked_sub(outflow).unwrap();
        Ok(())
    }

    pub fn update_rate_limit(
        &mut self,
        account_info: &AccountInfo,
        status: Option<ReserveStatus>,
        rate_limit_slope: Option<u64>,
        rate_limit_max_outflow: Option<u64>,
    ) -> Result<(), ProgramError> {
        
        if let Some(status) = status {
            self.status = status;
        }

        if let Some(rate_limit_slope) = rate_limit_slope {
            self.rate_limit_slope = rate_limit_slope;
        }

        if let Some(rate_limit_max_outflow) = rate_limit_max_outflow {
            self.rate_limit_max_outflow = rate_limit_max_outflow;
        }

        // Commit the account on-chain
        self.save(account_info)?;

        Ok(())
    }
    

    pub fn sync_balance(
        &mut self,
        vault_info: &AccountInfo,
        controller_info: &AccountInfo,
        controller: &Controller,
    ) -> Result<(), ProgramError> {

        if vault_info.key().ne(&self.vault) {
            return Err(ProgramError::InvalidAccountData)
        }
        if controller_info.key().ne(&self.controller) {
            return Err(ProgramError::InvalidAccountData)
        }

        // Get the current slot and time
        let clock = Clock::get()?;
        
        // Load in the vault, since it could have an opening balance
        let vault = TokenAccount::from_account_info(vault_info)?;
        let new_balance = vault.amount();

        if self.last_balance != new_balance {
            
            let previous_balance = self.last_balance;

            // Update the rate limits and balance for the change
            if new_balance > self.last_balance  { // => inflow
                self.update_for_inflow(
                    clock, 
                    new_balance.checked_sub(self.last_balance).unwrap()
                )?;
            } else { // new_balance < previous_balance => outflow (should not be possible)
                self.update_for_outflow(
                    clock, 
                    self.last_balance.checked_sub(new_balance).unwrap()
                )?;
            }

            controller.emit_event(
                controller_info,
                SvmAlmControllerEvent::AccountingEvent (
                    AccountingEvent {
                        controller: self.controller,
                        integration: Self::derive_pda(self.controller, self.mint).unwrap().0,
                        mint: self.mint,
                        action: AccountingAction::Sync,
                        before: previous_balance,
                        after: self.last_balance // (new balance after the update)
                    }
                )
            )?;

        }

        Ok(())

    }
}

