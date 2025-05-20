use crate::{constants::ORACLE_SEED, state::nova_account::NovaAccount};

use super::super::discriminator::{AccountDiscriminators, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
};
use shank::ShankAccount;

#[derive(Clone, Debug, PartialEq, ShankAccount, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Oracle {
    /// Type of Oracle (0 = Switchboard)
    pub oracle_type: u8,
    /// Address of price feed.
    pub price_feed: Pubkey,
    /// Reserved space.
    pub reserved: [u8; 32],
}

impl Discriminator for Oracle {
    const DISCRIMINATOR: u8 = AccountDiscriminators::Oracle as u8;
}

impl NovaAccount for Oracle {
    const LEN: usize = 96;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        let (pda, bump) =
            find_program_address(&[ORACLE_SEED, self.price_feed.as_ref()], &crate::ID);
        Ok((pda, bump))
    }
}
