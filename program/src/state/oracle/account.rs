use crate::{
    constants::ORACLE_SEED, processor::shared::create_pda_account, state::nova_account::NovaAccount,
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
use shank::ShankAccount;

#[derive(Clone, Debug, PartialEq, ShankAccount, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Oracle {
    /// Type of Oracle (0 = Switchboard)
    pub oracle_type: u8,
    /// Address of price feed.
    pub price_feed: Pubkey,
    /// Reserved space (e.g. for Pyth price update account)
    pub reserved: [u8; 64],
}

impl Discriminator for Oracle {
    const DISCRIMINATOR: u8 = AccountDiscriminators::Oracle as u8;
}

impl NovaAccount for Oracle {
    const LEN: usize = 97;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) =
            find_program_address(&[ORACLE_SEED, self.price_feed.as_ref()], &crate::ID);
        Ok((pda, bump))
    }
}

impl Oracle {
    pub fn init_account(
        account_info: &AccountInfo,
        payer_info: &AccountInfo,
        price_feed: &AccountInfo,
        oracle_type: u8,
    ) -> Result<Self, ProgramError> {
        // Create and serialize the oracle_config
        let oracle_config = Oracle {
            oracle_type,
            price_feed: *price_feed.key(),
            reserved: [0; 64],
        };

        // Derive the PDA
        let (pda, bump) = oracle_config.derive_pda()?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds); // PDA was invalid
        }

        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(ORACLE_SEED),
            Seed::from(price_feed.key()),
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
        oracle_config.save(account_info)?;
        Ok(oracle_config)
    }
}
