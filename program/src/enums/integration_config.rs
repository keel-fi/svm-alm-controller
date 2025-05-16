use crate::integrations::{
    cctp_bridge::config::CctpBridgeConfig, lz_bridge::config::LzBridgeConfig,
    spl_token_external::config::SplTokenExternalConfig, spl_token_swap::config::SplTokenSwapConfig,
    swap::config::AtomicSwapConfig,
};
use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;
use solana_keccak_hasher::hash;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationConfig {
    Undefined { _padding: [u8; 192] },
    SplTokenExternal(SplTokenExternalConfig),
    SplTokenSwap(SplTokenSwapConfig),
    CctpBridge(CctpBridgeConfig),
    LzBridge(LzBridgeConfig),
    AtomicSwap(AtomicSwapConfig),
}

impl IntegrationConfig {
    pub fn hash(&self) -> [u8; 32] {
        let serialized = self.try_to_vec().unwrap();
        hash(serialized.as_slice()).to_bytes()
    }
}
