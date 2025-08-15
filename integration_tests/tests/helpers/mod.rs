use constants::{
    CCTP_LOCAL_TOKEN, CCTP_MESSAGE_TRANSMITTER, CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID,
    CCTP_REMOTE_TOKEN_MESSENGER, CCTP_TOKEN_MESSENGER, CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
    CCTP_TOKEN_MINTER, LZ_USDS_OFT_PROGRAM_ID, LZ_USDS_OFT_STORE_PUBKEY,
    LZ_USDS_PEER_CONFIG_PUBKEY, NOVA_TOKEN_SWAP_PROGRAM_ID, USDC_TOKEN_MINT_PUBKEY,
    USDS_TOKEN_MINT_PUBKEY,
};
use litesvm::LiteSVM;
pub mod assert;
pub mod cctp;
pub mod constants;
pub mod logs_parser;
pub mod raydium;
pub mod spl;
use base64;
pub use logs_parser::print_inner_instructions;
use serde_json::Value;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use std::env;
use std::{fs, str::FromStr};

use crate::helpers::constants::{
    BONK_MINT, KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, KAMINO_MAIN_MARKET, KAMINO_REFERRER_METADATA, KAMINO_USDC_RESERVE, KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT, KAMINO_USDC_RESERVE_BONK_VAULT, KAMINO_USDC_RESERVE_COLLATERAL_MINT, KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY, KAMINO_USDC_RESERVE_FARM_COLLATERAL, KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY, KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, LZ_ENDPOINT_PROGRAM_ID, LZ_EXECUTOR_PROGRAM_ID, LZ_R1_PROGRAM_ID, LZ_R2_PROGRAM_ID, LZ_ULN302, LZ_USDS_ESCROW
};

/// Get LiteSvm with myproject loaded.
pub fn lite_svm_with_programs() -> LiteSVM {
    let mut svm = LiteSVM::new();

    // Add the CONTROLLER program
    let controller_program_bytes = include_bytes!("../../../target/deploy/svm_alm_controller.so");
    svm.add_program(
        svm_alm_controller_client::SVM_ALM_CONTROLLER_ID,
        controller_program_bytes,
    );

    // Add the NOVA TOKEN SWAP program
    let nova_token_swap_program_bytes = include_bytes!("../../fixtures/nova_token_swap.so");
    svm.add_program(NOVA_TOKEN_SWAP_PROGRAM_ID, nova_token_swap_program_bytes);

    // Add the CCTP Programs
    let cctp_message_transmitter_program =
        include_bytes!("../../fixtures/cctp_message_transmitter.so");
    svm.add_program(
        CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID,
        cctp_message_transmitter_program,
    );
    let cctp_token_messenger_minter_program =
        include_bytes!("../../fixtures/cctp_token_messenger_minter.so");
    svm.add_program(
        CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
        cctp_token_messenger_minter_program,
    );

    // Add the CCTP accounts
    let usdc_mint_account = get_account_data_from_json("./fixtures/usdc_mint.json");
    svm.set_account(USDC_TOKEN_MINT_PUBKEY, usdc_mint_account)
        .unwrap();
    let cctp_local_token_account = get_account_data_from_json("./fixtures/cctp_local_token.json");
    svm.set_account(CCTP_LOCAL_TOKEN, cctp_local_token_account)
        .unwrap();
    let cctp_message_transmitter_account =
        get_account_data_from_json("./fixtures/cctp_message_transmitter.json");
    svm.set_account(CCTP_MESSAGE_TRANSMITTER, cctp_message_transmitter_account)
        .unwrap();
    let cctp_token_messenger_account =
        get_account_data_from_json("./fixtures/cctp_token_messenger.json");
    svm.set_account(CCTP_TOKEN_MESSENGER, cctp_token_messenger_account)
        .unwrap();
    let cctp_token_minter_account = get_account_data_from_json("./fixtures/cctp_token_minter.json");
    svm.set_account(CCTP_TOKEN_MINTER, cctp_token_minter_account)
        .unwrap();
    let cctp_remote_token_messenger_account =
        get_account_data_from_json("./fixtures/cctp_remote_token_messenger.json");
    svm.set_account(
        CCTP_REMOTE_TOKEN_MESSENGER,
        cctp_remote_token_messenger_account,
    )
    .unwrap();

    // Layer Zero
    let usds_mint_account = get_account_data_from_json("./fixtures/usds_mint.json");
    svm.set_account(USDS_TOKEN_MINT_PUBKEY, usds_mint_account)
        .unwrap();
    let lz_usds_oft_store_account = get_account_data_from_json("./fixtures/lz_usds_oft_store.json");
    svm.set_account(LZ_USDS_OFT_STORE_PUBKEY, lz_usds_oft_store_account)
        .unwrap();
    let lz_usds_eth_peer_config_account =
        get_account_data_from_json("./fixtures/lz_usds_eth_peer_config.json");
    svm.set_account(LZ_USDS_PEER_CONFIG_PUBKEY, lz_usds_eth_peer_config_account)
        .unwrap();
    let usds_oft_program = include_bytes!("../../fixtures/lz_oft.so");
    svm.add_program(LZ_USDS_OFT_PROGRAM_ID, usds_oft_program);
    let lz_endpoint_program = include_bytes!("../../fixtures/lz_endpoint.so");
    svm.add_program(LZ_ENDPOINT_PROGRAM_ID, lz_endpoint_program);
    let lz_send_program = include_bytes!("../../fixtures/lz_send.so");
    svm.add_program(LZ_ULN302, lz_send_program);
    let lz_r1_program = include_bytes!("../../fixtures/lz_r1.so");
    svm.add_program(LZ_R1_PROGRAM_ID, lz_r1_program);
    let lz_r2_program = include_bytes!("../../fixtures/lz_r2.so");
    svm.add_program(LZ_R2_PROGRAM_ID, lz_r2_program);
    let lz_executor_program = include_bytes!("../../fixtures/lz_executor.so");
    svm.add_program(LZ_EXECUTOR_PROGRAM_ID, lz_executor_program);


    // Kamino Lend
    let kamino_lend_program = include_bytes!("../../fixtures/kamino_lend.so");
    svm.add_program(KAMINO_LEND_PROGRAM_ID, kamino_lend_program);
    let kamino_farms_program = include_bytes!("../../fixtures/kamino_farms.so");
    svm.add_program(KAMINO_FARMS_PROGRAM_ID, kamino_farms_program);
    let kamino_main_market_account = get_account_data_from_json("./fixtures/kamino_main_market.json");
    svm.set_account(KAMINO_MAIN_MARKET, kamino_main_market_account)
        .unwrap();
    let kamino_usdc_reserve = get_account_data_from_json("./fixtures/kamino_usdc_reserve.json");
    svm.set_account(KAMINO_USDC_RESERVE, kamino_usdc_reserve)
        .unwrap();
    let kamino_usdc_reserve_farm_collateral = get_account_data_from_json("./fixtures/usdc_reserve_farm_collateral.json");
    svm.set_account(KAMINO_USDC_RESERVE_FARM_COLLATERAL, kamino_usdc_reserve_farm_collateral)
        .unwrap();
    let kamino_referrer_user_metadata = get_account_data_from_json("./fixtures/kamino_referrer_metadata.json");
    svm.set_account(KAMINO_REFERRER_METADATA, kamino_referrer_user_metadata)
        .unwrap();
    let kamino_usdc_reserve_liquidity_supply = get_account_data_from_json("./fixtures/kamino_usdc_reserve_liquidity_supply.json");
    svm.set_account(KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY, kamino_usdc_reserve_liquidity_supply)
        .unwrap();
    let kamino_usdc_reserve_collateral_mint = get_account_data_from_json("./fixtures/kamino_usdc_reserve_collateral_mint.json");
    svm.set_account(KAMINO_USDC_RESERVE_COLLATERAL_MINT, kamino_usdc_reserve_collateral_mint)
        .unwrap();
    let kamino_usdc_reserve_collateral_supply = get_account_data_from_json("./fixtures/kamino_usdc_reserve_collateral_supply.json");
    svm.set_account(KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY, kamino_usdc_reserve_collateral_supply)
        .unwrap();
    let kamino_usdc_reserve_scope_config_price_feed = get_account_data_from_json("./fixtures/kamino_usdc_reserve_scope_config_price_feed.json");
    svm.set_account(KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, kamino_usdc_reserve_scope_config_price_feed)
        .unwrap();
    let kamino_usdc_reserve_farm_global_config = get_account_data_from_json("./fixtures/kamino_farm_global_config.json");
    svm.set_account(KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, kamino_usdc_reserve_farm_global_config)
        .unwrap();
    let bonk_mint = get_account_data_from_json("./fixtures/bonk_mint.json");
    svm.set_account(BONK_MINT, bonk_mint)
        .unwrap();
    let bonk_reward_vault = get_account_data_from_json("./fixtures/usdc_reserve_bonk_vault.json");
    svm.set_account(KAMINO_USDC_RESERVE_BONK_VAULT, bonk_reward_vault)
        .unwrap();
    let bonk_treasury_vaut = get_account_data_from_json("./fixtures/usdc_reserve_bonk_treasury_vault.json");
    svm.set_account(KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT, bonk_treasury_vaut)
        .unwrap();
    svm
}

fn get_account_data_from_json(path: &str) -> Account {
    let current_dir = env::current_dir().expect("Unable to get current directory");
    let json_data = fs::read_to_string(path).expect("Unable to read JSON file");
    let v: Value = serde_json::from_str(&json_data).expect("Unable to parse JSON");

    let lamports = v["account"]["lamports"]
        .as_u64()
        .expect("Expected lamports as u64");
    let base64_data = v["account"]["data"][0].as_str().expect("Expected a string");
    let data = base64::decode(base64_data).expect("Failed to decode base64");
    let owner_str = v["account"]["owner"]
        .as_str()
        .expect("Expected owner as string");
    let owner = Pubkey::from_str(owner_str).expect("Invalid owner pubkey");
    let executable = v["account"]["executable"]
        .as_bool()
        .expect("Expected executable as bool");
    let rent_epoch = v["account"]["rentEpoch"]
        .as_u64()
        .expect("Expected rentEpoch as u64");

    Account {
        lamports,
        data,
        owner,
        executable,
        rent_epoch,
    }
}
