use constants::{CCTP_LOCAL_TOKEN, CCTP_MESSAGE_TRANSMITTER, CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID, CCTP_REMOTE_TOKEN_MESSENGER, CCTP_TOKEN_MESSENGER, CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID, CCTP_TOKEN_MINTER, LZ_USDS_OFT_PROGRAM_ID, LZ_USDS_OFT_STORE_PUBKEY, LZ_USDS_PEER_CONFIG_PUBKEY, NOVA_TOKEN_SWAP_PROGRAM_ID, USDC_TOKEN_MINT_PUBKEY, USDS_TOKEN_MINT_PUBKEY, WORMHOLE_GUARDIAN_SET_4_PUBKEY};
use litesvm::LiteSVM;
pub mod logs_parser;
pub mod cctp;
pub mod constants;
pub use logs_parser::print_inner_instructions;
use serde_json::Value;
use solana_sdk::account::Account;
use std::{fs, str::FromStr};
use base64;
use solana_sdk::pubkey::Pubkey;
use std::env;

/// Get LiteSvm with myproject loaded.
pub fn lite_svm_with_programs() -> LiteSVM {
    
    let mut svm = LiteSVM::new();

    // Add the CONTROLLER program
    let controller_program_bytes = include_bytes!("../../../target/deploy/svm_alm_controller.so");
    svm.add_program(svm_alm_controller_client::programs::SVM_ALM_CONTROLLER_ID, controller_program_bytes);

    // Add the NOVA TOKEN SWAP program
    let nova_token_swap_program_bytes = include_bytes!("../../fixtures/nova_token_swap.so");
    svm.add_program(NOVA_TOKEN_SWAP_PROGRAM_ID, nova_token_swap_program_bytes);

    // // Get the Account object
    let gs4_account = get_account_data_from_json("./fixtures/wormhole_guardian_set_4.json");
    svm.set_account(WORMHOLE_GUARDIAN_SET_4_PUBKEY, gs4_account).unwrap();

 
    // Add the CCTP Programs
    let cctp_message_transmitter_program = include_bytes!("../../fixtures/cctp_message_transmitter.so");
    svm.add_program(CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID, cctp_message_transmitter_program);
    let cctp_token_messenger_minter_program = include_bytes!("../../fixtures/cctp_token_messenger_minter.so");
    svm.add_program(CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID, cctp_token_messenger_minter_program);

    // Add the CCTP accounts
    let usdc_mint_account = get_account_data_from_json("./fixtures/usdc_mint.json");
    svm.set_account(USDC_TOKEN_MINT_PUBKEY, usdc_mint_account).unwrap();
    let cctp_local_token_account = get_account_data_from_json("./fixtures/cctp_local_token.json");
    svm.set_account(CCTP_LOCAL_TOKEN, cctp_local_token_account).unwrap();
    let cctp_message_transmitter_account = get_account_data_from_json("./fixtures/cctp_message_transmitter.json");
    svm.set_account(CCTP_MESSAGE_TRANSMITTER, cctp_message_transmitter_account).unwrap();
    let cctp_token_messenger_account = get_account_data_from_json("./fixtures/cctp_token_messenger.json");
    svm.set_account(CCTP_TOKEN_MESSENGER, cctp_token_messenger_account).unwrap();
    let cctp_token_minter_account = get_account_data_from_json("./fixtures/cctp_token_minter.json");
    svm.set_account(CCTP_TOKEN_MINTER, cctp_token_minter_account).unwrap();
    let cctp_remote_token_messenger_account = get_account_data_from_json("./fixtures/cctp_remote_token_messenger.json");
    svm.set_account(CCTP_REMOTE_TOKEN_MESSENGER, cctp_remote_token_messenger_account).unwrap();

    // Layer Zero
    let usds_oft_program = include_bytes!("../../fixtures/oft.so");
    svm.add_program(LZ_USDS_OFT_PROGRAM_ID, usds_oft_program);
    let usds_mint_account = get_account_data_from_json("./fixtures/usds_mint.json");
    svm.set_account(USDS_TOKEN_MINT_PUBKEY, usds_mint_account).unwrap();
    let lz_usds_oft_store_account = get_account_data_from_json("./fixtures/lz_usds_oft_store.json");
    svm.set_account(LZ_USDS_OFT_STORE_PUBKEY, lz_usds_oft_store_account).unwrap();
    let lz_usds_eth_peer_config_account = get_account_data_from_json("./fixtures/lz_usds_eth_peer_config.json");
    svm.set_account(LZ_USDS_PEER_CONFIG_PUBKEY, lz_usds_eth_peer_config_account).unwrap();

    svm
}

fn get_account_data_from_json(path: &str) -> Account {

    let current_dir = env::current_dir().expect("Unable to get current directory");
    let json_data = fs::read_to_string(path).expect("Unable to read JSON file");
    let v: Value = serde_json::from_str(&json_data).expect("Unable to parse JSON");

    let lamports = v["account"]["lamports"].as_u64().expect("Expected lamports as u64");
    let base64_data = v["account"]["data"][0].as_str().expect("Expected a string");
    let data = base64::decode(base64_data).expect("Failed to decode base64");
    let owner_str = v["account"]["owner"].as_str().expect("Expected owner as string");
    let owner = Pubkey::from_str(owner_str).expect("Invalid owner pubkey");
    let executable = v["account"]["executable"].as_bool().expect("Expected executable as bool");
    let rent_epoch = v["account"]["rentEpoch"].as_u64().expect("Expected rentEpoch as u64");

    Account {
        lamports,
        data,
        owner,
        executable,
        rent_epoch,
    }
}
