use crate::{
    constants::ORACLE_SEED, error::SvmAlmControllerErrors, processor::shared::create_pda_account,
    state::keel_account::KeelAccount,
};

use super::super::discriminator::{AccountDiscriminators, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    msg,
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
    sysvars::{rent::Rent, Sysvar},
};
use shank::{ShankAccount, ShankType};
use switchboard_on_demand::{
    Discriminator as SwitchboardDiscriminator, PullFeedAccountData,
    SWITCHBOARD_ON_DEMAND_PROGRAM_ID,
};

#[derive(Clone, Debug, PartialEq, ShankType, Copy, BorshSerialize, BorshDeserialize)]
pub struct Feed {
    /// Address of price feed.
    pub price_feed: Pubkey,
    /// Type of Oracle (0 = Switchboard)
    pub oracle_type: u8,
    /// Reserved space (for additional context, transformations and operations).
    pub reserved: [u8; 63],
}

#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Oracle {
    /// Version of account layout (defaults to 1)
    pub version: u8,
    /// Authority required for update operations.
    pub authority: Pubkey,
    /// Nonce used as part of PDA seed.
    pub nonce: Pubkey,
    /// Price stored with full precision.
    pub value: i128,
    /// Precision of value.
    pub precision: u32,
    /// Slot in which value was last updated in the oracle feed.
    /// Note that this is not the slot in which prices were last refreshed.
    pub last_update_slot: u64,
    /// Controller the Oracle belongs to.
    pub controller: Pubkey,
    /// Mint that the oracle quotes price for. This is mainly used
    /// to avoid possible footguns when initializing integrations
    /// that utilize the given Oracle.
    pub mint: Pubkey,
    /// Extra space reserved before feeds array.
    pub reserved: [u8; 64],
    /// Price feeds.
    pub feeds: [Feed; 1],
}

impl Discriminator for Oracle {
    const DISCRIMINATOR: u8 = AccountDiscriminators::OracleDiscriminator as u8;
}

impl KeelAccount for Oracle {
    const LEN: usize = 317;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        try_find_program_address(&[ORACLE_SEED, self.nonce.as_ref()], &crate::ID)
            .ok_or(ProgramError::InvalidSeeds)
    }
}

impl Oracle {
    /// Validate that the Oracle is a supported Oracle [Switchboard].
    pub fn verify_oracle_type(
        oracle_type: u8,
        price_feed: &AccountInfo,
    ) -> Result<(), ProgramError> {
        match oracle_type {
            0 => {
                if !price_feed.is_owned_by(&SWITCHBOARD_ON_DEMAND_PROGRAM_ID.to_bytes()) {
                    return Err(SvmAlmControllerErrors::InvalidAccountData.into());
                }

                let feed_account = price_feed.try_borrow_data()?;
                if !feed_account.starts_with(&PullFeedAccountData::discriminator()) {
                    msg!("Invalid PullFeedAccount discriminator");
                    return Err(ProgramError::InvalidAccountData);
                };

                // Deserialize account to check it's correct
                let _feed: &PullFeedAccountData = bytemuck::try_from_bytes(&feed_account[8..])
                    .map_err(|_| ProgramError::InvalidAccountData)?;

                Ok(())
            }
            _ => Err(SvmAlmControllerErrors::UnsupportedOracleType.into()),
        }
    }

    pub fn check_data(
        &self,
        controller: Option<&Pubkey>,
        authority: Option<&Pubkey>,
    ) -> Result<(), ProgramError> {
        // Controller validation is not always required. Specifically, the
        // RefreshOracle instruction does not require Controller checks.
        if let Some(controller) = controller {
            if self.controller.ne(controller) {
                msg!("Controller does not match Oracle controller");
                return Err(SvmAlmControllerErrors::ControllerDoesNotMatchAccountData.into());
            }
        }
        // Authority validation is not needed for every instruction the
        // Oracle is loaded, so we only validate when passed as arg.
        if let Some(authority) = authority {
            if self.authority.ne(authority) {
                msg!("Oracle authority mismatch");
                return Err(ProgramError::IncorrectAuthority);
            }
        }
        Ok(())
    }

    pub fn load_and_check(
        account_info: &AccountInfo,
        controller: Option<&Pubkey>,
        authority: Option<&Pubkey>,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        let oracle: Self = KeelAccount::deserialize(&account_info.try_borrow_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        oracle.check_data(controller, authority)?;
        oracle.verify_pda(account_info)?;
        Ok(oracle)
    }

    pub fn init_account(
        account_info: &AccountInfo,
        authority_info: &AccountInfo,
        payer_info: &AccountInfo,
        controller: &Pubkey,
        mint: &Pubkey,
        nonce: &Pubkey,
        oracle_type: u8,
        price_feed: &AccountInfo,
    ) -> Result<Self, ProgramError> {
        let precision = match oracle_type {
            0 => Ok::<u32, ProgramError>(switchboard_on_demand::on_demand::PRECISION),
            _ => Err(SvmAlmControllerErrors::UnsupportedOracleType.into()),
        }?;

        // Create and serialize the oracle
        let oracle = Oracle {
            version: 1,
            authority: *authority_info.key(),
            nonce: *nonce,
            value: 0,
            precision,
            last_update_slot: 0,
            controller: *controller,
            mint: *mint,
            reserved: [0; 64],
            feeds: [Feed {
                oracle_type,
                price_feed: *price_feed.key(),
                reserved: [0; 63],
            }],
        };

        // Derive the PDA
        let (pda, bump) = oracle.derive_pda()?;
        if account_info.key().ne(&pda) {
            msg!("Oracle PDA mismatch");
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(ORACLE_SEED),
            Seed::from(nonce),
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
        oracle.save(account_info)?;
        Ok(oracle)
    }

    /// Get the Oracle's price allowing for inversion.
    ///
    /// Let P = precision of price and X = Price in decimals
    /// Price is stored in data feed as X * (10^P).
    /// By inverting, we want to get 1/X * (10^P)
    /// = 10^P / X = 10^(2*P) / (X * 10^P)
    pub fn get_price(&self, invert: bool) -> i128 {
        if invert {
            10_i128
                .checked_pow(self.precision * 2)
                .unwrap()
                .checked_div(self.value)
                .unwrap()
        } else {
            self.value
        }
    }
}
