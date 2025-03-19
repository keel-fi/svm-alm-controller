use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;
use crate::integrations::{
    spl_token_external::state::SplTokenExternalState, 
    spl_token_swap::state::SplTokenSwapState, 
    spl_token_vault::state::SplTokenVaultState
};

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationState {
    Undefined {
        _padding: [u8; 48]
    },
    SplTokenVault(SplTokenVaultState),
    SplTokenExternal(SplTokenExternalState),
    SplTokenSwap(SplTokenSwapState)
}




