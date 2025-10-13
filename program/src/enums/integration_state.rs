use crate::integrations::{
    atomic_swap::state::AtomicSwapState, cctp_bridge::state::CctpBridgeState,
    drift::state::DriftState, lz_bridge::state::LzBridgeState,
    spl_token_external::state::SplTokenExternalState,
    kamino::state::KaminoState,
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
    Drift(DriftState),
    Kamino(KaminoState),
}
