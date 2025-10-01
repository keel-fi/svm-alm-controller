use super::{fetch_reserve_account, get_token_balance_or_zero};
use crate::{
    helpers::constants::{DEVNET_RPC, LZ_ENDPOINT_PROGRAM_ID, LZ_USDS_ESCROW},
    subs::{
        derive_controller_authority_pda, derive_permission_pda, derive_reserve_pda,
        get_mint_supply_or_zero,
    },
};
use borsh::BorshDeserialize;
use litesvm::{
    types::{FailedTransactionMetadata, TransactionResult},
    LiteSVM,
};
use oft_client::{
    instructions::SendInstructionArgs,
    oft302::{
        Oft302, Oft302Accounts, Oft302Programs, Oft302QuoteParams, Oft302SendAccounts,
        Oft302SendPrograms,
    },
};
use solana_keccak_hasher::hash;
use solana_program::pubkey;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program, sysvar,
    transaction::Transaction,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use std::error::Error;
use svm_alm_controller_client::generated::{
    accounts::{Integration, Reserve},
    instructions::{
        InitializeIntegrationBuilder, ManageIntegrationBuilder, PushBuilder,
        ResetLzPushInFlightBuilder,
    },
    programs::SVM_ALM_CONTROLLER_ID,
    types::{InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType, PushArgs},
};

pub fn derive_integration_pda(controller_pda: &Pubkey, hash: &[u8; 32]) -> Pubkey {
    let (integration_pda, _integration_bump) = Pubkey::find_program_address(
        &[b"integration", &controller_pda.to_bytes(), &hash.as_ref()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    integration_pda
}

pub fn fetch_integration_account(
    svm: &LiteSVM,
    integration_pda: &Pubkey,
) -> Result<Option<Integration>, Box<dyn Error>> {
    let info = svm.get_account(integration_pda);
    match info {
        Some(info) => {
            if info.data.is_empty() {
                Ok(None)
            } else {
                Integration::try_from_slice(&info.data[1..])
                    .map(Some)
                    .map_err(Into::into)
            }
        }
        None => Ok(None),
    }
}

pub fn initialize_integration(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    payer: &Keypair,
    authority: &Keypair,
    description: &str,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    permit_liquidation: bool,
    config: &IntegrationConfig,
    inner_args: &InitializeArgs,
    skip_assertions: bool,
) -> Result<Pubkey, FailedTransactionMetadata> {
    let calling_permission_pda = derive_permission_pda(controller, &authority.pubkey());
    let controller_authority = derive_controller_authority_pda(controller);

    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

    let hash = hash(borsh::to_vec(config).unwrap().as_ref()).to_bytes();
    let integration_type = match config {
        IntegrationConfig::AtomicSwap(_) => IntegrationType::AtomicSwap,
        _ => panic!("Not specified"),
    };

    let remaining_accounts: &[AccountMeta] = match config {
        IntegrationConfig::AtomicSwap(c) => &[
            AccountMeta {
                pubkey: c.input_token,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.output_token,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.oracle,
                is_signer: false,
                is_writable: false,
            },
        ],
        _ => panic!("Not specified"),
    };

    let integration_pda = derive_integration_pda(controller, &hash);

    let ixn = InitializeIntegrationBuilder::new()
        .integration_type(integration_type)
        .status(status)
        .description(description_encoding)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .permit_liquidation(permit_liquidation)
        .inner_args(inner_args.clone())
        .payer(payer.pubkey())
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .integration(integration_pda)
        .add_remaining_accounts(remaining_accounts)
        .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&authority, &payer],
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn);

    if skip_assertions {
        return tx_result.map(|_| integration_pda);
    }

    if tx_result.is_err() {
        println!("{:#?}", tx_result.unwrap().logs);
    } else {
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    let integration = fetch_integration_account(svm, &integration_pda)
        .expect("Failed to fetch integration account");
    assert!(
        integration.is_some(),
        "Integration must exist after the transaction"
    );

    let integration = integration.unwrap();
    assert_eq!(
        integration.status, status,
        "Status does not match expected value"
    );
    assert_eq!(
        integration.rate_limit_slope, rate_limit_slope,
        "Rate limit slope does not match expected value"
    );
    assert_eq!(
        integration.rate_limit_max_outflow, rate_limit_max_outflow,
        "Rate limit max outflow does not match expected value"
    );
    assert_eq!(
        integration.controller, *controller,
        "Controller does not match expected value"
    );
    assert_eq!(
        integration.config, *config,
        "Config does not match expected value"
    );

    Ok(integration_pda)
}

pub fn manage_integration(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    integration: &Pubkey,
    authority: &Keypair,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
) -> Result<(), Box<dyn Error>> {
    let calling_permission_pda = derive_permission_pda(controller, &authority.pubkey());
    let controller_authority = derive_controller_authority_pda(controller);

    let ixn = ManageIntegrationBuilder::new()
        .status(status)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .integration(*integration)
        .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn);
    if tx_result.is_err() {
        println!("{:#?}", tx_result.unwrap().logs);
    } else {
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    let integration =
        fetch_integration_account(svm, integration).expect("Failed to fetch integration account");
    assert!(
        integration.is_some(),
        "Integration must exist after the transaction"
    );

    let integration = integration.unwrap();
    assert_eq!(
        integration.status, status,
        "Status does not match expected value"
    );
    assert_eq!(
        integration.rate_limit_slope, rate_limit_slope,
        "Rate limit slope does not match expected value"
    );
    assert_eq!(
        integration.rate_limit_max_outflow, rate_limit_max_outflow,
        "Rate limit max outflow does not match expected value"
    );
    assert_eq!(
        integration.controller, *controller,
        "Controller does not match expected value"
    );

    Ok(())
}
