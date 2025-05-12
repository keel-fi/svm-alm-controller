extern crate alloc;
use super::discriminator::{AccountDiscriminators, Discriminator};
use crate::{
    constants::{INTEGRATION_SEED, SECONDS_PER_DAY},
    enums::{IntegrationConfig, IntegrationState, IntegrationStatus},
    processor::shared::create_pda_account,
    state::nova_account::NovaAccount,
};
use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
};
use shank::ShankAccount;
use solana_program::pubkey::Pubkey as SolanaPubkey;

#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Integration {
    pub controller: Pubkey,
    pub description: [u8; 32],
    pub hash: [u8; 32],
    pub lookup_table: Pubkey,
    pub status: IntegrationStatus,
    pub rate_limit_slope: u64,
    pub rate_limit_max_outflow: u64,
    pub rate_limit_amount_last_update: u64,
    pub last_refresh_timestamp: i64,
    pub last_refresh_slot: u64,
    pub config: IntegrationConfig,
    pub state: IntegrationState,
}

impl Discriminator for Integration {
    const DISCRIMINATOR: u8 = AccountDiscriminators::IntegrationDiscriminator as u8;
}

impl NovaAccount for Integration {
    const LEN: usize = 4 * 32 + 1 + 193 + 49 + 8 * 5;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) = SolanaPubkey::find_program_address(
            &[
                INTEGRATION_SEED,
                self.controller.as_ref(),
                self.hash.as_ref(),
            ],
            &SolanaPubkey::from(crate::ID),
        );
        Ok((pda.to_bytes(), bump))
    }
}

impl Integration {
    fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        // Check discriminator
        if data[0] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        // Use Borsh deserialization
        Self::try_from_slice(&data[1..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn check_data(&self, controller: &Pubkey) -> Result<(), ProgramError> {
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
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        // Check PDA

        let integration = Self::deserialize(&account_info.try_borrow_data()?).unwrap();
        integration.check_data(controller)?;
        integration.verify_pda(account_info)?;
        Ok(integration)
    }

    pub fn load_and_check_mut(
        account_info: &AccountInfo,
        controller: &Pubkey,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        let integration = Self::deserialize(&account_info.try_borrow_mut_data()?).unwrap();
        integration.check_data(controller)?;
        integration.verify_pda(account_info)?;
        Ok(integration)
    }

    pub fn save(&self, account_info: &AccountInfo) -> Result<(), ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
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
        description: [u8; 32],
        lookup_table: Pubkey,
        rate_limit_slope: u64,
        rate_limit_max_outflow: u64,
    ) -> Result<Self, ProgramError> {
        let clock = Clock::get()?;
        // Derive the hash for this config
        let hash = config.hash();

        // Create and serialize the controller
        let integration = Integration {
            controller,
            hash,
            status,
            lookup_table,
            description,
            config,
            state,
            rate_limit_slope,
            rate_limit_max_outflow,
            rate_limit_amount_last_update: rate_limit_max_outflow,
            last_refresh_timestamp: clock.unix_timestamp,
            last_refresh_slot: clock.slot,
        };

        // Derive the PDA
        let (pda, bump) = integration.derive_pda()?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds.into()); // PDA was invalid
        }

        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(INTEGRATION_SEED),
            Seed::from(&controller),
            Seed::from(&hash),
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
        integration.save(account_info)?;

        Ok(integration)
    }

    pub fn update_and_save(
        &mut self,
        account_info: &AccountInfo,
        status: Option<IntegrationStatus>,
        lookup_table: Option<Pubkey>,
        rate_limit_slope: Option<u64>,
        rate_limit_max_outflow: Option<u64>,
    ) -> Result<(), ProgramError> {
        // Need to refresh rate limits before any updates
        let clock = Clock::get()?;
        self.refresh_rate_limit(clock)?;

        if let Some(status) = status {
            self.status = status;
        }
        if let Some(lookup_table) = lookup_table {
            self.lookup_table = lookup_table;
        }
        if let Some(rate_limit_slope) = rate_limit_slope {
            self.rate_limit_slope = rate_limit_slope;
        }
        if let Some(rate_limit_max_outflow) = rate_limit_max_outflow {
            let gap = self
                .rate_limit_max_outflow
                .checked_sub(self.rate_limit_amount_last_update)
                .unwrap();
            self.rate_limit_max_outflow = rate_limit_max_outflow;
            // Reset the rate_limit_amount_last_update such that the gap from the max remains the same
            self.rate_limit_amount_last_update = self.rate_limit_max_outflow.saturating_sub(gap);
        }

        // Commit the account on-chain
        self.save(account_info)?;

        Ok(())
    }

    pub fn refresh_rate_limit(&mut self, clock: Clock) -> Result<(), ProgramError> {
        if self.rate_limit_max_outflow == u64::MAX
            || self.last_refresh_timestamp == clock.unix_timestamp
        {
            () // Do nothing
        } else {
            self.rate_limit_amount_last_update = self
                .rate_limit_amount_last_update
                .checked_add(
                    (self.rate_limit_slope as u128
                        * clock
                            .unix_timestamp
                            .checked_sub(self.last_refresh_timestamp)
                            .unwrap() as u128
                        / SECONDS_PER_DAY as u128) as u64,
                )
                .unwrap_or(self.rate_limit_max_outflow);
        }
        self.last_refresh_timestamp = clock.unix_timestamp;
        self.last_refresh_slot = clock.slot;
        Ok(())
    }

    pub fn update_rate_limit_for_inflow(
        &mut self,
        clock: Clock,
        inflow: u64,
    ) -> Result<(), ProgramError> {
        if !(self.last_refresh_timestamp == clock.unix_timestamp
            && self.last_refresh_slot == clock.slot)
        {
            msg! {"Rate limit must be refreshed before updating for flows"}
            return Err(ProgramError::InvalidArgument);
        }
        // Cap the rate_limit_amount_last_update at the rate_limit_max_outflow
        let v = self.rate_limit_amount_last_update.saturating_add(inflow);
        if v > self.rate_limit_max_outflow {
            // Cannot daily max outflow
            self.rate_limit_amount_last_update = self.rate_limit_max_outflow;
        } else {
            self.rate_limit_amount_last_update = v;
        }
        Ok(())
    }

    pub fn update_rate_limit_for_outflow(
        &mut self,
        clock: Clock,
        outflow: u64,
    ) -> Result<(), ProgramError> {
        if !(self.last_refresh_timestamp == clock.unix_timestamp
            && self.last_refresh_slot == clock.slot)
        {
            msg! {"Rate limit must be refreshed before updating for flows"}
            return Err(ProgramError::InvalidArgument);
        }
        self.rate_limit_amount_last_update = self
            .rate_limit_amount_last_update
            .checked_sub(outflow)
            .unwrap();
        Ok(())
    }
}
