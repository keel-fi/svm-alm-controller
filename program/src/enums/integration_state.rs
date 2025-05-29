use crate::integrations::{
    atomic_swap::state::AtomicSwapState, cctp_bridge::state::CctpBridgeState,
    lz_bridge::state::LzBridgeState, spl_token_external::state::SplTokenExternalState,
    spl_token_swap::state::SplTokenSwapState,
};
use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationState {
    Undefined { _padding: [u8; 48] },
    SplTokenExternal(SplTokenExternalState),
    SplTokenSwap(SplTokenSwapState),
    CctpBridge(CctpBridgeState),
    LzBridge(LzBridgeState),
    AtomicSwap(AtomicSwapState),
}
