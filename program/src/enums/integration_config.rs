use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio_log::log;
use shank::ShankType;
use solana_keccak_hasher::hash;
use crate::integrations::spl_token_vault::config::SplTokenVaultConfig;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationConfig {
    Undefined {
        _padding: [u8; 192]
    },
    SplTokenVault(SplTokenVaultConfig)
    // SplTokenVault {
    //     program: Pubkey,
    //     inbound_token_account: Pubkey, // ATA
    //     vault_token_account: Pubkey,
    //     token_mint: Pubkey,
    //     _padding: [u8; 64]
    // },
    // SplTokenExternal {
    //     program: Pubkey,
    //     authority: Pubkey,
    //     token_account: Pubkey,
    //     token_mint: Pubkey,
    //     _padding: [u8; 64]
    // },
    // SwapIntent {
    //     program: Pubkey,
    //     from_token_mint: Pubkey,
    //     to_token_mint: Pubkey,
    //     oracle: Pubkey,
    //     _padding: [u8; 64]
    // },
    // CctpBridge {
    //     program: Pubkey,
    //     token_mint: Pubkey,
    //     inbount_token_account: Pubkey,
    //     destination_domain: u32,
    //     destination_address: Pubkey,
    //     _padding: [u8; 60]
    // },
    // LayerZeroBridge {
    //     program: Pubkey,
    //     token_mint: Pubkey,
    //     inbount_token_account: Pubkey,
    //     destination_domain: u32,
    //     mint_recipient: Pubkey,
    //     destination_caller: Pubkey,
    //     _padding: [u8; 28]
    // }
}

impl IntegrationConfig {

    pub fn hash(&self) -> [u8; 32] {
        let serialized = self.try_to_vec().unwrap();
        hash(serialized.as_slice()).to_bytes()
    }

}

