use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;
use solana_keccak_hasher::hash;
use crate::integrations::{
    cctp_bridge::config::CctpBridgeConfig, 
    spl_token_external::config::SplTokenExternalConfig, 
    spl_token_swap::config::SplTokenSwapConfig,
    lz_bridge::config::LzBridgeConfig
};

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationConfig {
    Undefined {_padding: [u8; 192]},
    SplTokenExternal(SplTokenExternalConfig),
    SplTokenSwap(SplTokenSwapConfig),
    CctpBridge(CctpBridgeConfig),
    LzBridge(LzBridgeConfig),

}

impl IntegrationConfig {

    pub fn hash(&self) -> [u8; 32] {
        let serialized = self.try_to_vec().unwrap();
        hash(serialized.as_slice()).to_bytes()
    }

}

