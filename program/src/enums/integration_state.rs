use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;
use crate::integrations::spl_token_vault::state::SplTokenVaultState;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationState {
    Undefined {
        _padding: [u8; 32]
    },
    SplTokenVault(SplTokenVaultState),
}




