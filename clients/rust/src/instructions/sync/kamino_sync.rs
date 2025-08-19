use solana_sdk::{
    instruction::{AccountMeta, Instruction}, 
    pubkey::Pubkey, 
    signature::Keypair, 
    signer::Signer, 
    system_program
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

use crate::{
    constants::{
        ASSOCIATED_TOKEN_PROGRAM_ID, 
        KAMINO_FARMS_PROGRAM_ID
    }, 
    generated::{instructions::SyncBuilder, types::KaminoConfig}, 
    pdas::{
        derive_controller_authority_pda, 
        derive_farm_vaults_authority, 
        derive_obligation_farm_address, 
        derive_reserve_pda, 
        derive_rewards_treasury_vault, 
        derive_rewards_vault
    }, 
    SVM_ALM_CONTROLLER_ID
};

pub fn get_kamino_sync_ix(
    controller: &Pubkey,
    integration: &Pubkey,
    authority: &Keypair,
    kamino_config: &KaminoConfig,
    rewards_mint: &Pubkey,
    global_config: &Pubkey,
    rewards_ata: &Pubkey,
    scope_prices: &Pubkey,
    rewards_token_program: &Pubkey,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);

    let vault = get_associated_token_address_with_program_id(
        &controller_authority,
        &kamino_config.reserve_liquidity_mint,
        &spl_token::ID,
    );
    let obligation = kamino_config.obligation;
    let kamino_reserve = kamino_config.reserve;
    let kamino_reserve_liquidity_mint = kamino_config.reserve_liquidity_mint;
    let reserve_pda = derive_reserve_pda(controller, &kamino_reserve_liquidity_mint);
    let reserve_farm = &kamino_config.reserve_farm_collateral;
    let obligation_farm_pda = derive_obligation_farm_address(
        reserve_farm, 
        &obligation, 
    );
    let rewards_vault_pda = derive_rewards_vault(
        reserve_farm, 
        &rewards_mint, 
    );                        
    let rewards_treasury_vault_pda = derive_rewards_treasury_vault(
        &global_config, 
        &rewards_mint, 
    );

    let farms_vault_authority_pda = derive_farm_vaults_authority(
        reserve_farm, 
    );
    
    let remaining_accounts = &[
        AccountMeta {
            pubkey: vault,
            is_writable: true,
            is_signer: false,
        },
        AccountMeta {
            pubkey: kamino_reserve,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: obligation,
            is_signer: false,
            is_writable: false
        },
        AccountMeta {
            pubkey: obligation_farm_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *reserve_farm,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: rewards_vault_pda,
            is_signer: false,
            is_writable: true
        },
        AccountMeta {
            pubkey: rewards_treasury_vault_pda,
            is_signer: false,
            is_writable: true
        },
        AccountMeta {
            pubkey: farms_vault_authority_pda,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *global_config,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *rewards_ata,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *rewards_mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *scope_prices,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *rewards_token_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: KAMINO_FARMS_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: system_program::ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: ASSOCIATED_TOKEN_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        }
    ];
    
    SyncBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .integration(*integration)
        .reserve(reserve_pda)
        .program_id(SVM_ALM_CONTROLLER_ID)
        .add_remaining_accounts(remaining_accounts)
        .instruction()
}