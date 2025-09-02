use super::discriminator::{AccountDiscriminators, Discriminator};
use crate::{
    constants::INTEGRATION_SEED,
    enums::{IntegrationConfig, IntegrationState, IntegrationStatus},
    error::SvmAlmControllerErrors,
    processor::shared::{calculate_rate_limit_increment, create_pda_account},
    state::keel_account::KeelAccount,
};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    msg,
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
};
use shank::ShankAccount;

/// Integrations enable controlled interactions with third-party DeFi protocols, however there are also
/// a number of `Integration` “special cases” — namely, to support:
/// - Transferring balances to an external wallet
/// - Facilitating atomic swapping between tokens
/// - Bridging specific tokens using canonical bridges (CCTP, LayerZero OFT)
///
/// Integration accounts stores the necessary use-case specific configurations to enforce account contexts in
/// CPIs to the relevant third party protocol(s), and stores the data necessary to support Integration-level
/// rate limiting, and use-case specific data to facilitate accounting.
#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Integration {
    /// Controller the Integration belongs to
    pub controller: Pubkey,
    pub description: [u8; 32],
    /// Hash of the Integration's IntegrationConfig to be used as a PDA seed
    pub hash: [u8; 32],
    /// Address Lookup Table associated with this Integration (set to Pubkey::default() when not needed)
    pub lookup_table: Pubkey,
    /// Status of the Integration (i.e. active or suspended)
    pub status: IntegrationStatus,
    /// The number of units (i.e. TokenAccount amount) is replenished to the available amount per 24 hours
    pub rate_limit_slope: u64,
    /// The cap of tokens that may outflow (i.e. Integration "Pushes") on a rolling window basis
    pub rate_limit_max_outflow: u64,
    /// The current amount of tokens able to outflow (i.e. Integration "Pushes") on a rolling window basis
    pub rate_limit_outflow_amount_available: u64,
    /// Remainder from previous refresh. This is necessary to avoid DOS
    /// of the Reserve via the rate limit when the `rate_limit_max_outflow`
    /// is set to a low value requiring longer time lapses before incrementing
    /// the available outflow amount.
    pub rate_limit_remainder: u64,
    /// Timestamp when the Integration was last updated
    pub last_refresh_timestamp: i64,
    /// The Solana slot where the Integration was last updated
    pub last_refresh_slot: u64,
    /// Configuration for the specific type of Integration with a third party program
    pub config: IntegrationConfig,
    /// Integration specific state (i.e. LP balances)
    pub state: IntegrationState,
    pub _padding: [u8; 56],
}

impl Discriminator for Integration {
    const DISCRIMINATOR: u8 = AccountDiscriminators::IntegrationDiscriminator as u8;
}

impl KeelAccount for Integration {
    const LEN: usize = 4 * 32 + 1 + 6 * 8 + 225 + 49 + 56;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        try_find_program_address(
            &[
                INTEGRATION_SEED,
                self.controller.as_ref(),
                self.hash.as_ref(),
            ],
            &crate::ID,
        )
        .ok_or(ProgramError::InvalidSeeds)
    }
}

impl Integration {
    pub fn check_data(&self, controller: &Pubkey) -> Result<(), ProgramError> {
        if self.controller.ne(controller) {
            msg!("Controller does not match Integration controller");
            return Err(SvmAlmControllerErrors::ControllerDoesNotMatchAccountData.into());
        }
        Ok(())
    }

    pub fn load_and_check(
        account_info: &AccountInfo,
        controller: &Pubkey,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        // Check PDA

        let integration: Self = KeelAccount::deserialize(&account_info.try_borrow_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        integration.check_data(controller)?;
        integration.verify_pda(account_info)?;
        Ok(integration)
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
            rate_limit_outflow_amount_available: rate_limit_max_outflow,
            rate_limit_remainder: 0,
            last_refresh_timestamp: clock.unix_timestamp,
            last_refresh_slot: clock.slot,
            _padding: [0; 56],
        };

        // Derive the PDA
        let (pda, bump) = integration.derive_pda()?;
        if account_info.key().ne(&pda) {
            msg!("Integration PDA mismatch");
            return Err(SvmAlmControllerErrors::InvalidPda.into()); // PDA was invalid
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
            Self::DISCRIMINATOR_SIZE + Self::LEN,
            &crate::ID,
            account_info,
            &signer_seeds,
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
        description: Option<[u8; 32]>,
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
        if let Some(description) = description {
            self.description = description;
        }
        if let Some(rate_limit_max_outflow) = rate_limit_max_outflow {
            let gap = self
                .rate_limit_max_outflow
                .checked_sub(self.rate_limit_outflow_amount_available)
                .unwrap();
            self.rate_limit_max_outflow = rate_limit_max_outflow;
            // Reset the rate_limit_outflow_amount_available such that the gap from the max remains the same
            self.rate_limit_outflow_amount_available =
                self.rate_limit_max_outflow.saturating_sub(gap);
        }

        // Commit the account on-chain
        self.save(account_info)?;

        Ok(())
    }

    pub fn refresh_rate_limit(&mut self, clock: Clock) -> Result<(), ProgramError> {
        if self.rate_limit_max_outflow != u64::MAX
            && self.last_refresh_timestamp != clock.unix_timestamp
        {
            let (increment, remainder) = calculate_rate_limit_increment(
                clock.unix_timestamp,
                self.last_refresh_timestamp,
                self.rate_limit_slope,
                self.rate_limit_remainder,
            );
            self.rate_limit_outflow_amount_available = self
                .rate_limit_outflow_amount_available
                .saturating_add(increment)
                .min(self.rate_limit_max_outflow);
            self.rate_limit_remainder = remainder;
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
        // Cap the rate_limit_outflow_amount_available at the rate_limit_max_outflow
        let v = self
            .rate_limit_outflow_amount_available
            .saturating_add(inflow);
        if v > self.rate_limit_max_outflow {
            // Cannot daily max outflow
            self.rate_limit_outflow_amount_available = self.rate_limit_max_outflow;
        } else {
            self.rate_limit_outflow_amount_available = v;
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
        self.rate_limit_outflow_amount_available = self
            .rate_limit_outflow_amount_available
            .checked_sub(outflow)
            .ok_or(SvmAlmControllerErrors::RateLimited)?;
        Ok(())
    }
}
