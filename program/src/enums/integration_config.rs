use crate::integrations::{
    atomic_swap::config::AtomicSwapConfig, 
    cctp_bridge::config::CctpBridgeConfig, 
    drift::config::DriftConfig, 
    kamino::config::KaminoConfig, 
    lz_bridge::config::LzBridgeConfig, 
    psm_swap::config::PsmSwapConfig, 
    spl_token_external::config::SplTokenExternalConfig
};
use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;
use solana_keccak_hasher::hash;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationConfig {
    Undefined { _padding: [u8; 224] },
    SplTokenExternal(SplTokenExternalConfig),
    CctpBridge(CctpBridgeConfig),
    LzBridge(LzBridgeConfig),
    AtomicSwap(AtomicSwapConfig),
    Drift(DriftConfig),
    Kamino(KaminoConfig),
    PsmSwap(PsmSwapConfig)
}

impl IntegrationConfig {
    pub fn hash(&self) -> [u8; 32] {
        let serialized = self.try_to_vec().unwrap();
        hash(serialized.as_slice()).to_bytes()
    }
}
