use crate::integrations::{
    atomic_swap::state::AtomicSwapState, 
    cctp_bridge::state::CctpBridgeState, 
    lz_bridge::state::LzBridgeState, 
    psm_swap::state::PsmSwapState, 
    shared::lending_markets::LendingState, 
    spl_token_external::state::SplTokenExternalState
};
use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationState {
    Undefined { _padding: [u8; 48] },
    SplTokenExternal(SplTokenExternalState),
    CctpBridge(CctpBridgeState),
    LzBridge(LzBridgeState),
    AtomicSwap(AtomicSwapState),
    Drift(LendingState),
    Kamino(LendingState),
    PsmSwap(PsmSwapState),
}
