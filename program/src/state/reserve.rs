use super::{
    discriminator::{AccountDiscriminators, Discriminator},
    keel_account::KeelAccount,
    Controller,
};
use crate::{
    constants::RESERVE_SEED,
    enums::ReserveStatus,
    error::SvmAlmControllerErrors,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    processor::shared::{calculate_rate_limit_increment, create_pda_account},
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
use pinocchio_token_interface::TokenAccount;
use shank::ShankAccount;

/// The Reserve account manages a specific TokenAccount ultimately owned by a specific Controller's
/// authority PDA. The Reserve enforces certain policies like outflow rate limiting.
#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Reserve {
    /// Controller the Reserve belongs to
    pub controller: Pubkey,
    /// Token Mint of the Reserve
    pub mint: Pubkey,
    /// TokenAccount holding the Reserve's tokens
    pub vault: Pubkey,
    /// Status of the Reserve (i.e. active or suspended)
    pub status: ReserveStatus,
    /// The number of units (i.e. TokenAccount amount) is replenished to the available amount per 24 hours
    pub rate_limit_slope: u64,
    /// The cap of tokens that may outflow on a rolling window basis
    pub rate_limit_max_outflow: u64,
    /// The current amount of tokens able to outflow on a rolling window basis
    pub rate_limit_outflow_amount_available: u64,
    /// Remainder from previous refresh. This is necessary to avoid DOS
    /// of the Reserve via the rate limit when the `rate_limit_max_outflow`
    /// is set to a low value requiring longer time lapses before incrementing
    /// the available outflow amount.
    pub rate_limit_remainder: u64,
    /// The last recorded balance of the Reserve's vault TokenAccount
    pub last_balance: u64,
    /// Timestamp when the Reserve was last updated
    pub last_refresh_timestamp: i64,
    /// The Solana slot where the Reserve was last updated
    pub last_refresh_slot: u64,
    pub _padding: [u8; 120],
}

impl Discriminator for Reserve {
    const DISCRIMINATOR: u8 = AccountDiscriminators::ReserveDiscriminator as u8;
}

impl KeelAccount for Reserve {
    const LEN: usize = 3 * 32 + 1 + 7 * 8 + 120;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        try_find_program_address(
            &[RESERVE_SEED, self.controller.as_ref(), self.mint.as_ref()],
            &crate::ID,
        )
        .ok_or(ProgramError::InvalidSeeds)
    }
}

impl Reserve {
    pub fn check_data(&self, controller: &Pubkey) -> Result<(), ProgramError> {
        if self.controller.ne(controller) {
            msg!("Controller does not match Reserve controller");
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

        let reserve: Self = KeelAccount::deserialize(&account_info.try_borrow_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        reserve.check_data(controller)?;
        reserve.verify_pda(account_info)?;
        Ok(reserve)
    }

    /// Initializes the PDA account for a reserve.
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
        // Create and serialize the controller
        let clock = Clock::get()?;
        let reserve = Reserve {
            controller,
            mint,
            vault,
            status,
            rate_limit_slope,
            rate_limit_max_outflow,
            rate_limit_outflow_amount_available: rate_limit_max_outflow, // Starts at full amount
            last_balance: 0,
            last_refresh_timestamp: clock.unix_timestamp,
            last_refresh_slot: clock.slot,
            rate_limit_remainder: 0,
            _padding: [0; 120],
        };
        // Derive the PDA
        let (pda, bump) = reserve.derive_pda()?;
        if account_info.key().ne(&pda) {
            msg!("Reserve PDA mismatch");
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }
        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(RESERVE_SEED),
            Seed::from(&controller),
            Seed::from(&mint),
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
        reserve.save(account_info)?;
        Ok(reserve)
    }

    pub fn update(
        &mut self,
        status: Option<ReserveStatus>,
        rate_limit_slope: Option<u64>,
        rate_limit_max_outflow: Option<u64>,
    ) -> Result<(), ProgramError> {
        // Need to refresh rate limits before any updates
        let clock = Clock::get()?;
        self.refresh_rate_limit(clock)?;

        if let Some(status) = status {
            self.status = status;
        }
        if let Some(rate_limit_slope) = rate_limit_slope {
            self.rate_limit_slope = rate_limit_slope;
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
            if gap > self.rate_limit_max_outflow {
                // Reset remainder during update
                self.rate_limit_remainder = 0;
            }
        }
        Ok(())
    }

    /// Refresh the rate limit amount based on the slope and the time since the last refresh.
    /// If the rate limit is set to `u64::MAX`, it will not refresh.
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
            if self.rate_limit_outflow_amount_available == self.rate_limit_max_outflow {
                self.rate_limit_remainder = 0;
            } else {
                self.rate_limit_remainder = remainder;
            }
        }

        self.last_refresh_timestamp = clock.unix_timestamp;
        self.last_refresh_slot = clock.slot;
        Ok(())
    }

    /// Increment the rate limit amount for inflows and update the last balance by the amount received.
    pub fn update_for_inflow(&mut self, clock: Clock, inflow: u64) -> Result<(), ProgramError> {
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
        self.last_balance = self.last_balance.checked_add(inflow).unwrap();
        Ok(())
    }

    /// Decrement the rate limit amount for outflows and update the last balance by the amount sent.
    /// NOTE: Due to Token2022 and PermanentDelegate extensions, it's possible for the outflow
    /// to be larger than the available amount. In this scenario, we must skip the underflow check
    /// in order to allow operations to proceed.
    pub fn update_for_outflow(
        &mut self,
        clock: Clock,
        outflow: u64,
        allow_underflow: bool,
    ) -> Result<(), ProgramError> {
        if !(self.last_refresh_timestamp == clock.unix_timestamp
            && self.last_refresh_slot == clock.slot)
        {
            msg! {"Rate limit must be refreshed before updating for flows"}
            return Err(ProgramError::InvalidArgument);
        }

        // Under certain conditions, we prevent erroring on underflow.
        if allow_underflow {
            self.rate_limit_outflow_amount_available = self
                .rate_limit_outflow_amount_available
                .saturating_sub(outflow);
        } else {
            self.rate_limit_outflow_amount_available = self
                .rate_limit_outflow_amount_available
                .checked_sub(outflow)
                .ok_or(SvmAlmControllerErrors::RateLimited)?;
        }
        self.last_balance = self.last_balance.checked_sub(outflow).unwrap();
        Ok(())
    }

    /// Sync the balance of the reserve with the vault and update the rate limits whether there is an inflow or outflow.
    /// Also refreshes the rate limits based on time since the last refresh.
    pub fn sync_balance(
        &mut self,
        vault_info: &AccountInfo,
        controller_authority_info: &AccountInfo,
        controller_key: &Pubkey,
        controller: &Controller,
    ) -> Result<(), ProgramError> {
        if vault_info.key().ne(&self.vault) {
            msg!("Vault does not match Reserve vault");
            return Err(ProgramError::InvalidAccountData);
        }
        if controller_key.ne(&self.controller) {
            msg!("Controller does not match Reserve controller");
            return Err(SvmAlmControllerErrors::ControllerDoesNotMatchAccountData.into());
        }

        // Get the current slot and time
        let clock = Clock::get()?;

        // Refresh the rate limits
        self.refresh_rate_limit(clock)?;

        // Load in the vault, since it could have an opening balance
        let vault = TokenAccount::from_account_info(vault_info)?;
        let new_balance = vault.amount();
        drop(vault);

        if self.last_balance != new_balance {
            let previous_balance = self.last_balance;
            let abs_delta = new_balance.abs_diff(previous_balance);

            // Update the rate limits and balance for the change
            let direction = if new_balance > self.last_balance {
                // => inflow
                self.update_for_inflow(clock, abs_delta)?;
                AccountingDirection::Credit
            } else {
                // new_balance < previous_balance => outflow (possible with Token2022 and PermanentDelegate extension)
                self.update_for_outflow(
                    clock, abs_delta,
                    true, // Allow underflow since this can happen with Token2022 and PermanentDelegate
                )?;
                AccountingDirection::Debit
            };

            controller.emit_event(
                controller_authority_info,
                controller_key,
                SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                    controller: self.controller,
                    integration: None,
                    reserve: Some(self.derive_pda()?.0),
                    mint: self.mint,
                    action: AccountingAction::Sync,
                    delta: abs_delta,
                    direction,
                }),
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reserve_update_for_outflow_allow_underflow() {
        let mut reserve = Reserve {
            controller: Pubkey::default(),
            mint: Pubkey::default(),
            vault: Pubkey::default(),
            status: ReserveStatus::Active,
            rate_limit_slope: 100,
            rate_limit_max_outflow: 1000,
            rate_limit_outflow_amount_available: 500,
            rate_limit_remainder: 0,
            last_balance: 1000,
            last_refresh_timestamp: 0,
            last_refresh_slot: 0,
            _padding: [0; 120],
        };

        let default_clock = Clock::from_bytes(&[0u8; Clock::LEN]).unwrap();
        reserve
            .update_for_outflow(*default_clock, 600, true)
            .unwrap();
        assert_eq!(
            reserve.rate_limit_outflow_amount_available, 0,
            "Should clamp to 0 with allow_underflow"
        );

        assert!(
            reserve
                .update_for_outflow(*default_clock, 600, false)
                .is_err(),
            "Should error on underflow without allow_underflow"
        );
    }
}
