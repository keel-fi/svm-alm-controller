use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use bytemuck::{Pod, Zeroable};
use pinocchio::pubkey::Pubkey;

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct PsmPool {
    /// Status of the pool.
    pub status: u8,
    pub bump: u8,
    pub salt: u16,
    pub config_authority: Pubkey,
    pub freeze_authority: Pubkey,
    pub pricing_authorities: [PricingAuthorityConfig; 3],
    pub liquidity_owner: Pubkey,
    pub tokens_count: u64,
    pub pairs_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable, Default)]
#[repr(C, packed)]
pub struct PricingAuthorityConfig {
    pub pricing_authority: Pubkey,
    pub status: u8,
}

impl AccountZerocopyDeserialize<1> for PsmPool {
    const DISCRIMINATOR: [u8; 1] = [1];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(C, packed)]
pub struct Token {
    pub mint: Pubkey,
    pub pool: Pubkey,
    pub vault: Pubkey,
    /// Status of the token (0 = Active, 1 = Suspended)
    pub status: u8,
    /// Maximum allowed inflow for this token
    pub token_max_inflow: u64,
    /// Maximum allowed outflow for this token
    pub token_max_outflow: u64,
    /// Total inflow since last reset
    pub token_inflow_since_reset: u64,
    /// Total outflow since last reset
    pub token_outflow_since_reset: u64,
}

impl AccountZerocopyDeserialize<1> for Token {
    const DISCRIMINATOR: [u8; 1] = [2];
}
