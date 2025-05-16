use super::{
    super::discriminator::{AccountDiscriminators, Discriminator},
    pyth_config::PythConfig,
};
use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankAccount;

// Provides flexibility for future Oracle configurations or more complex types.
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum OracleConfig {
    PythFeed(PythConfig),
}

#[derive(Clone, Debug, PartialEq, ShankAccount, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Oracle {
    /// The block the oracle information was last updated in.
    pub last_updated_block: u64,
    /// The up-to-date numerator for a price of a given asset
    pub price_numerator: u64,
    /// The up-to-date denonomiator for a price of a given asset. Use of denom numerator and
    /// denominator make normalization and decimal handling easier.
    pub price_denominator: u64,
    /// The configuration for this oracle.
    pub config: OracleConfig,
}

impl Discriminator for Oracle {
    const DISCRIMINATOR: u8 = AccountDiscriminators::Oracle as u8;
}

impl Oracle {
    pub const LEN: usize = 96;
}
