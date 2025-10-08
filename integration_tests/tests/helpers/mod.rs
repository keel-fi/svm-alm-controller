use constants::{
    CCTP_LOCAL_TOKEN, CCTP_MESSAGE_TRANSMITTER, CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID,
    CCTP_REMOTE_TOKEN_MESSENGER, CCTP_TOKEN_MESSENGER, CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
    CCTP_TOKEN_MINTER, LZ_USDS_OFT_PROGRAM_ID, LZ_USDS_OFT_STORE_PUBKEY,
    LZ_USDS_PEER_CONFIG_PUBKEY, USDC_TOKEN_MINT_PUBKEY,
    USDS_TOKEN_MINT_PUBKEY,
};
use litesvm::LiteSVM;
pub mod assert;
pub mod cctp;
pub mod constants;
pub mod invalid_account_testing;
pub mod lite_svm;
pub mod lz_oft;
pub mod macros;
pub mod raydium;
pub mod spl;
pub mod utils;
pub use macros::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use svm_alm_controller_client::generated::types::{ControllerStatus, PermissionStatus};

use crate::helpers::constants::{
    LZ_ENDPOINT_PROGRAM_ID, LZ_EXECUTOR_PROGRAM_ID, LZ_R1_PROGRAM_ID, LZ_R2_PROGRAM_ID, LZ_ULN302, KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID,
};
use crate::helpers::lite_svm::get_account_data_from_json;
use crate::subs::{airdrop_lamports, initialize_contoller, manage_permission};

/// Get LiteSvm with myproject loaded.
pub fn lite_svm_with_programs() -> LiteSVM {
    let mut svm = LiteSVM::new();

    // Add the CONTROLLER program
    let controller_program_bytes = include_bytes!("../../../target/deploy/svm_alm_controller.so");
    svm.add_program(
        svm_alm_controller_client::SVM_ALM_CONTROLLER_ID,
        controller_program_bytes,
    );

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

    svm
}

#[allow(dead_code)]
pub struct TestContext {
    pub svm: LiteSVM,
    pub super_authority: Keypair,
    pub controller_pk: Pubkey,
}

#[allow(dead_code)]
pub fn setup_test_controller() -> Result<TestContext, Box<dyn std::error::Error>> {
    let mut svm = lite_svm_with_programs();

    let super_authority = Keypair::new();

    // Airdrop to payer
    airdrop_lamports(&mut svm, &super_authority.pubkey(), 1_000_000_000)?;

    let (controller_pk, _) = initialize_contoller(
        &mut svm,
        &super_authority,
        &super_authority,
        ControllerStatus::Active,
        321u16, // Id
    )?;

    // Update the authority to have all permissions
    let _ = manage_permission(
        &mut svm,
        &controller_pk,
        &super_authority,          // payer
        &super_authority,          // calling authority
        &super_authority.pubkey(), // subject authority
        PermissionStatus::Active,
        true, // can_execute_swap,
        true, // can_manage_permissions,
        true, // can_invoke_external_transfer,
        true, // can_reallocate,
        true, // can_freeze,
        true, // can_unfreeze,
        true, // can_manage_reserves_and_integrations
        true, // can_suspend_permissions
        true, // can_liquidate
    )?;

    Ok(TestContext {
        svm,
        super_authority,
        controller_pk,
    })
}
