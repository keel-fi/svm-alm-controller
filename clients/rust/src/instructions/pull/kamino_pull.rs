use solana_sdk::{
    instruction::{AccountMeta, Instruction}, 
    pubkey::Pubkey, 
    sysvar
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

use crate::{
    constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID}, 
    generated::{instructions::PullBuilder, types::{KaminoConfig, PullArgs}}, 
    pdas::{
        derive_controller_authority_pda, 
        derive_market_authority_address, 
        derive_obligation_farm_address, 
        derive_permission_pda, 
        derive_reserve_collateral_mint, 
        derive_reserve_collateral_supply, 
        derive_reserve_liquidity_supply, 
        derive_reserve_pda
    }, 
    SVM_ALM_CONTROLLER_ID
};

pub fn get_kamino_pull_ix(
    controller: &Pubkey,
    integration: &Pubkey,
    authority: &Pubkey,
    kamino_config: &KaminoConfig,
    amount: u64
) -> Instruction {
    let calling_permission_pda = derive_permission_pda(controller, authority);
    let controller_authority = derive_controller_authority_pda(controller);
    let obligation = kamino_config.obligation;
    let reserve_farm_collateral = kamino_config.reserve_farm_collateral;
    let kamino_reserve = kamino_config.reserve;
    let kamino_market = kamino_config.market;
    let kamino_reserve_liquidity_mint = kamino_config.reserve_liquidity_mint;
    let kamino_reserve_liquidity_supply = derive_reserve_liquidity_supply(
        &kamino_market, 
        &kamino_reserve_liquidity_mint, 
    );
    let kamino_reserve_collateral_mint = derive_reserve_collateral_mint(
        &kamino_market, 
        &kamino_reserve_liquidity_mint, 
    );
    let kamino_reserve_collateral_supply = derive_reserve_collateral_supply(
        &kamino_market, 
        &kamino_reserve_liquidity_mint, 
    );
    let market_authority = derive_market_authority_address(
        &kamino_market, 
    );
    let obligation_farm_collateral = derive_obligation_farm_address(
        &reserve_farm_collateral, 
        &obligation, 
    );

    let reserve_pda = derive_reserve_pda(controller, &kamino_reserve_liquidity_mint);
    let vault = get_associated_token_address_with_program_id(
        &controller_authority,
        &kamino_reserve_liquidity_mint,
        &spl_token::ID,
    ); 

    let remaining_accounts = &[
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true
        },
        AccountMeta {
            pubkey: obligation,
            is_signer: false,
            is_writable: true
        },
        AccountMeta {
            pubkey: kamino_reserve,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: kamino_reserve_liquidity_mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: kamino_reserve_liquidity_supply,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: kamino_reserve_collateral_mint,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: kamino_reserve_collateral_supply,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: market_authority,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: kamino_market,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: spl_token::ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: spl_token::ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: sysvar::instructions::ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: obligation_farm_collateral,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: reserve_farm_collateral,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: KAMINO_FARMS_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: KAMINO_LEND_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        }
    ];

    PullBuilder::new()
        .pull_args(PullArgs::Kamino { amount })
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .integration(*integration)
        .reserve_a(reserve_pda)
        .reserve_b(reserve_pda)
        .program_id(SVM_ALM_CONTROLLER_ID)
        .add_remaining_accounts(remaining_accounts)
        .instruction()
}