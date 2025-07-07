use crate::{
    constants::ORACLE_SEED, error::SvmAlmControllerErrors, processor::shared::create_pda_account,
    state::nova_account::NovaAccount,
};

use super::super::discriminator::{AccountDiscriminators, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{rent::Rent, Sysvar},
};
use shank::{ShankAccount, ShankType};
use switchboard_on_demand::{
    Discriminator as SwitchboardDiscriminator, PullFeedAccountData,
    SWITCHBOARD_ON_DEMAND_PROGRAM_ID,
};

#[derive(Clone, Debug, PartialEq, ShankType, BorshSerialize, BorshDeserialize)]
pub struct Feed {
    /// Address of price feed.
    pub price_feed: Pubkey,
    /// Type of Oracle (0 = Switchboard)
    pub oracle_type: u8,
    /// Transformations to apply
    pub invert_price: bool,
    /// Reserved space (for additional context, transformations and operations).
    pub reserved: [u8; 62],
}

#[derive(Clone, Debug, PartialEq, ShankAccount, BorshSerialize, BorshDeserialize)]
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
    /// Extra space reserved before feeds array.
    pub reserved: [u8; 64],
    /// Price feeds.
    pub feeds: [Feed; 1],
}

impl Discriminator for Oracle {
    const DISCRIMINATOR: u8 = AccountDiscriminators::OracleDiscriminator as u8;
}

impl NovaAccount for Oracle {
    const LEN: usize = 253;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) = find_program_address(&[ORACLE_SEED, self.nonce.as_ref()], &crate::ID);
        Ok((pda, bump))
    }
}

impl Oracle {
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
                    return Err(ProgramError::InvalidAccountData);
                };
                let _feed: &PullFeedAccountData = bytemuck::from_bytes(&feed_account[8..]);

                Ok(())
            }
            _ => Err(SvmAlmControllerErrors::UnsupportedOracleType.into()),
        }
    }

    pub fn load_and_check(account_info: &AccountInfo) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        let oracle: Self = NovaAccount::deserialize(&account_info.try_borrow_data()?).unwrap();
        oracle.verify_pda(account_info)?;
        Ok(oracle)
    }

    pub fn load_and_check_mut(account_info: &AccountInfo) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        let oracle: Self = NovaAccount::deserialize(&account_info.try_borrow_mut_data()?).unwrap();
        oracle.verify_pda(account_info)?;
        Ok(oracle)
    }

    pub fn init_account(
        account_info: &AccountInfo,
        authority_info: &AccountInfo,
        payer_info: &AccountInfo,
        nonce: &Pubkey,
        oracle_type: u8,
        price_feed: &AccountInfo,
        invert_price: bool,
    ) -> Result<Self, ProgramError> {
        // Create and serialize the oracle
        let oracle = Oracle {
            version: 1,
            authority: *authority_info.key(),
            nonce: *nonce,
            value: 0,
            precision: 0,
            last_update_slot: 0,
            reserved: [0; 64],
            feeds: [Feed {
                oracle_type,
                price_feed: *price_feed.key(),
                invert_price,
                reserved: [0; 62],
            }],
        };

        // Derive the PDA
        let (pda, bump) = oracle.derive_pda()?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds); // PDA was invalid
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
            1 + Self::LEN,
            &crate::ID,
            account_info,
            signer_seeds,
        )?;

        // Commit the account on-chain
        oracle.save(account_info)?;
        Ok(oracle)
    }
}
