use super::{fetch_reserve_account, get_token_balance_or_zero};
use crate::{
    helpers::{
        cctp::CctpDepositForBurnPdas,
        constants::{
            DEVNET_RPC, LZ_ENDPOINT_PROGRAM_ID, LZ_USDS_ESCROW, NOVA_TOKEN_SWAP_FEE_OWNER,
        },
    },
    subs::{
        derive_controller_authority_pda, derive_permission_pda, derive_reserve_pda, derive_swap_authority_pda_and_bump, get_mint_supply_or_zero
    },
};
use borsh::{BorshDeserialize, BorshSerialize};
use litesvm::{types::TransactionResult, LiteSVM};
use oft_client::{
    instructions::SendInstructionArgs,
    oft302::{
        Oft302, Oft302Accounts, Oft302Programs, Oft302QuoteParams, Oft302SendAccounts,
        Oft302SendPrograms,
    },
};
use solana_client::rpc_client::RpcClient;
use solana_keccak_hasher::hash;
use solana_program::pubkey;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    msg,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program, sysvar,
    transaction::Transaction,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller::state::controller;
use std::error::Error;
use svm_alm_controller_client::generated::{
    accounts::{Integration, Reserve},
    instructions::{
        InitializeIntegrationBuilder, ManageIntegrationBuilder, PullBuilder, PushBuilder,
    },
    programs::SVM_ALM_CONTROLLER_ID,
    types::{
        InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType, PullArgs, PushArgs,
        SplTokenExternalConfig,
    },
};

pub fn derive_integration_pda(controller_pda: &Pubkey, hash: &[u8; 32]) -> Pubkey {
    let (integration_pda, _integration_bump) = Pubkey::find_program_address(
        &[b"integration", &controller_pda.to_bytes(), &hash.as_ref()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    integration_pda
}

pub fn fetch_integration_account(
    svm: &mut LiteSVM,
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
    config: &IntegrationConfig,
    inner_args: &InitializeArgs,
) -> Result<Pubkey, Box<dyn Error>> {
    let calling_permission_pda = derive_permission_pda(controller, &authority.pubkey());
    let controller_authority = derive_controller_authority_pda(controller);

    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

    let (integration_type, hash) = match config {
        IntegrationConfig::SplTokenExternal(c) => {
            let b: Vec<u8> = [
                &[1u8][..],
                &c.program.to_bytes()[..],
                &c.mint.to_bytes()[..],
                &c.recipient.to_bytes()[..],
                &c.token_account.to_bytes()[..],
                &c.padding[..],
            ]
            .concat();
            let h = hash(b.as_slice()).to_bytes();
            (IntegrationType::SplTokenExternal, h)
        }
        IntegrationConfig::SplTokenSwap(c) => {
            let b: Vec<u8> = [
                &[2u8][..],
                &c.program.to_bytes()[..],
                &c.swap.to_bytes()[..],
                &c.mint_a.to_bytes()[..],
                &c.mint_b.to_bytes()[..],
                &c.lp_mint.to_bytes()[..],
                &c.lp_token_account.to_bytes()[..],
                &c.padding[..],
            ]
            .concat();
            let h = hash(b.as_slice()).to_bytes();
            (IntegrationType::SplTokenSwap, h)
        }
        IntegrationConfig::CctpBridge(c) => {
            let b: Vec<u8> = [
                &[3u8][..],
                &c.cctp_token_messenger_minter.to_bytes()[..],
                &c.cctp_message_transmitter.to_bytes()[..],
                &c.mint.to_bytes()[..],
                &c.destination_address.to_bytes()[..],
                &c.destination_domain.to_le_bytes()[..],
                &c.padding[..],
            ]
            .concat();
            let h = hash(b.as_slice()).to_bytes();
            (IntegrationType::CctpBridge, h)
        }
        IntegrationConfig::LzBridge(c) => {
            let b: Vec<u8> = [
                &[4u8][..],
                &c.program.to_bytes()[..],
                &c.mint.to_bytes()[..],
                &c.oft_store.to_bytes()[..],
                &c.peer_config.to_bytes()[..],
                &c.token_escrow.to_bytes()[..],
                &c.destination_address.to_bytes()[..],
                &c.destination_eid.to_le_bytes()[..],
                &c.padding[..],
            ]
            .concat();
            let h = hash(b.as_slice()).to_bytes();
            (IntegrationType::LzBridge, h)
        }
        IntegrationConfig::AtomicSwap(c) => {
            let mut serialized = Vec::with_capacity(std::mem::size_of::<IntegrationConfig>());
            config.serialize(&mut serialized).unwrap();
            let h = hash(serialized.as_slice()).to_bytes();
            (IntegrationType::AtomicSwap, h)
        }
        _ => panic!("Not specified"),
    };

    let remaining_accounts: &[AccountMeta] = match config {
        IntegrationConfig::SplTokenExternal(c) => &[
            AccountMeta {
                pubkey: c.mint,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.recipient,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.token_account,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: c.program,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pinocchio_associated_token_account::ID.into(),
                is_signer: false,
                is_writable: false,
            },
        ],
        IntegrationConfig::SplTokenSwap(c) => {
            let mint_a_token_program = pinocchio_token::ID.into();
            let mint_b_token_program = pinocchio_token::ID.into();
            let lp_mint_token_program = pinocchio_token::ID.into();
            let (swap_authority, _) = derive_swap_authority_pda_and_bump(&c.swap, &c.program);
            let swap_token_a = get_associated_token_address_with_program_id(
                &swap_authority,
                &c.mint_a,
                &mint_a_token_program,
            );
            let swap_token_b = get_associated_token_address_with_program_id(
                &swap_authority,
                &c.mint_b,
                &mint_b_token_program,
            );
            &[
                AccountMeta {
                    pubkey: c.swap,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: c.mint_a,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: c.mint_b,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: c.lp_mint,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: c.lp_token_account,
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: mint_a_token_program,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: mint_b_token_program,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: lp_mint_token_program,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: swap_token_a,
                    is_signer: false,
                    is_writable: false,
                }, // swap_token_a
                AccountMeta {
                    pubkey: swap_token_b,
                    is_signer: false,
                    is_writable: false,
                }, // swap_token_b
                AccountMeta {
                    pubkey: c.program,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: pinocchio_associated_token_account::ID.into(),
                    is_signer: false,
                    is_writable: false,
                },
            ]
        }
        IntegrationConfig::CctpBridge(c) => {
            let cctp_accounts = CctpDepositForBurnPdas::derive(
                c.cctp_message_transmitter,
                c.cctp_token_messenger_minter,
                c.mint,
                c.destination_domain,
            );
            &[
                AccountMeta {
                    pubkey: c.mint,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: cctp_accounts.local_token,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: cctp_accounts.remote_token_messenger,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: c.cctp_message_transmitter,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: c.cctp_token_messenger_minter,
                    is_signer: false,
                    is_writable: false,
                },
            ]
        }
        IntegrationConfig::LzBridge(c) => &[
            AccountMeta {
                pubkey: c.mint,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.oft_store,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.peer_config,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.program,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: c.token_escrow,
                is_signer: false,
                is_writable: false,
            },
        ],
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

    print!("hash: {:?}", hash);

    let integration_pda = derive_integration_pda(controller, &hash);

    let ixn = InitializeIntegrationBuilder::new()
        .integration_type(integration_type)
        .status(status)
        .description(description_encoding)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .inner_args(inner_args.clone())
        .payer(payer.pubkey())
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .integration(integration_pda)
        .lookup_table(system_program::ID) // TODO: Add this in the future
        .add_remaining_accounts(remaining_accounts)
        .system_program(system_program::ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&authority, &payer],
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn);
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
        .lookup_table(system_program::ID)
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

pub async fn push_integration(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    integration: &Pubkey,
    authority: &Keypair,
    push_args: &PushArgs,
) -> Result<TransactionResult, Box<dyn Error>> {
    let calling_permission_pda = derive_permission_pda(controller, &authority.pubkey());
    let controller_authority = derive_controller_authority_pda(controller);

    let integration_account = fetch_integration_account(svm, integration)
        .expect("Failed to fetch integration account")
        .unwrap();

    // Ixns to postpend to transaction.
    let mut post_ixns: Vec<Instruction> = vec![];

    // To support checks after
    let integration_before = fetch_integration_account(svm, &integration)
        .expect("Failed to fetch reserve account")
        .unwrap();
    let mut reserve_a_before: Option<Reserve> = None;
    let mut reserve_b_before: Option<Reserve> = None;
    let mut vault_a_balance_before = 0u64;
    let mut vault_b_balance_before = 0u64;
    let mut other_value_before = 0u64;
    match &integration_account.config {
        IntegrationConfig::SplTokenExternal(ref c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault =
                get_associated_token_address_with_program_id(&controller_authority, &c.mint, &c.program);
            reserve_a_before =
                fetch_reserve_account(svm, &reserve_pda).expect("Failed to fetch reserve account");
            vault_a_balance_before = get_token_balance_or_zero(svm, &vault);
            other_value_before = get_token_balance_or_zero(svm, &c.token_account);
            println!("{:?}", reserve_a_before);
            println!("{:?}", integration_before);
        }
        IntegrationConfig::SplTokenSwap(ref c) => {
            let reserve_a_pda = derive_reserve_pda(controller, &c.mint_a);
            let reserve_b_pda = derive_reserve_pda(controller, &c.mint_b);
            let token_program_a = Pubkey::from(pinocchio_token::ID);
            let token_program_b = Pubkey::from(pinocchio_token::ID);
            let vault_a = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_a,
                &token_program_a,
            );
            let vault_b = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_b,
                &token_program_b,
            );
            reserve_a_before = fetch_reserve_account(svm, &reserve_a_pda)
                .expect("Failed to fetch reserve account");
            reserve_b_before = fetch_reserve_account(svm, &reserve_b_pda)
                .expect("Failed to fetch reserve account");
            vault_a_balance_before = get_token_balance_or_zero(svm, &vault_a);
            vault_b_balance_before = get_token_balance_or_zero(svm, &vault_b);
            other_value_before = get_token_balance_or_zero(svm, &c.lp_token_account);
            println!("{:?}", reserve_a_before);
            println!("{:?}", reserve_b_before);
            println!("{:?}", other_value_before);
            println!("{:?}", integration_before);
        }
        IntegrationConfig::CctpBridge(ref c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint,
                &pinocchio_token::ID.into(),
            );
            reserve_a_before =
                fetch_reserve_account(svm, &reserve_pda).expect("Failed to fetch reserve account");
            vault_a_balance_before = get_token_balance_or_zero(svm, &vault);
            other_value_before = get_mint_supply_or_zero(svm, &c.mint);
            println!("{:?}", reserve_a_before);
            println!("{:?}", integration_before);
            println!("{:?}", other_value_before);
        }
        IntegrationConfig::LzBridge(ref c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint,
                &pinocchio_token::ID.into(),
            );
            reserve_a_before =
                fetch_reserve_account(svm, &reserve_pda).expect("Failed to fetch reserve account");
            vault_a_balance_before = get_token_balance_or_zero(svm, &vault);
            other_value_before = get_mint_supply_or_zero(svm, &c.mint);
            println!("{:?}", reserve_a_before);
            println!("{:?}", integration_before);
            println!("{:?}", other_value_before);
        }
        _ => panic!("Not configured"),
    };

    let (reserve_a_pk, reserve_b_pk, remaining_accounts, additional_signers): (
        Pubkey,
        Pubkey,
        &[AccountMeta],
        &[Keypair],
    ) = match &integration_account.config {
        IntegrationConfig::SplTokenExternal(ref c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault =
                get_associated_token_address_with_program_id(&controller_authority, &c.mint, &c.program);
            (
                reserve_pda,
                reserve_pda, // pass same reserve twice
                &[
                    AccountMeta {
                        pubkey: Pubkey::from(c.mint),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: Pubkey::from(vault),
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: Pubkey::from(c.recipient),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: Pubkey::from(c.token_account),
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: Pubkey::from(c.program),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: Pubkey::from(pinocchio_associated_token_account::ID),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: Pubkey::from(system_program::ID),
                        is_signer: false,
                        is_writable: false,
                    },
                ],
                &[],
            )
        }
        IntegrationConfig::SplTokenSwap(ref c) => {
            let reserve_a_pda = derive_reserve_pda(controller, &c.mint_a);
            let reserve_b_pda = derive_reserve_pda(controller, &c.mint_b);
            let token_program_a = Pubkey::from(pinocchio_token::ID);
            let token_program_b = Pubkey::from(pinocchio_token::ID);
            let token_program_lp = Pubkey::from(pinocchio_token::ID);
            let vault_a = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_a,
                &token_program_a,
            );
            let vault_b = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_b,
                &token_program_b,
            );
            let vault_lp = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.lp_mint,
                &token_program_lp,
            );
            let (swap_authority, _) = derive_swap_authority_pda_and_bump(&c.swap, &c.program);
            let swap_token_a = get_associated_token_address_with_program_id(
                &swap_authority,
                &c.mint_a,
                &token_program_a,
            );
            let swap_token_b = get_associated_token_address_with_program_id(
                &swap_authority,
                &c.mint_b,
                &token_program_b,
            );
            let swap_fee_account = get_associated_token_address_with_program_id(
                &NOVA_TOKEN_SWAP_FEE_OWNER,
                &c.lp_mint,
                &token_program_lp,
            );
            (
                reserve_a_pda,
                reserve_b_pda,
                &[
                    AccountMeta {
                        pubkey: c.swap,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: c.mint_a,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: c.mint_b,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: c.lp_mint,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: c.lp_token_account,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: token_program_a,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: token_program_b,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: token_program_lp,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: swap_token_a,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: swap_token_b,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: vault_a,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: vault_b,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: c.program,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: pinocchio_associated_token_account::ID.into(),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: swap_authority,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: swap_fee_account,
                        is_signer: false,
                        is_writable: true,
                    },
                ],
                &[],
            )
        }
        IntegrationConfig::CctpBridge(c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint,
                &pinocchio_token::ID.into(),
            );
            let cctp_accounts = CctpDepositForBurnPdas::derive(
                c.cctp_message_transmitter,
                c.cctp_token_messenger_minter,
                c.mint,
                c.destination_domain,
            );
            let message_sent_event_data = Keypair::new();
            (
                reserve_pda,
                reserve_pda, // repeat since only one required
                &[
                    AccountMeta {
                        pubkey: c.mint,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: vault,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: cctp_accounts.sender_authority,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: cctp_accounts.message_transmitter,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: cctp_accounts.token_messenger,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: cctp_accounts.remote_token_messenger,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: cctp_accounts.token_minter,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: cctp_accounts.local_token,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: message_sent_event_data.pubkey(),
                        is_signer: true,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: c.cctp_message_transmitter,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: c.cctp_token_messenger_minter,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: cctp_accounts.event_authority,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: pinocchio_token::ID.into(),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: system_program::ID,
                        is_signer: false,
                        is_writable: false,
                    },
                ],
                &[message_sent_event_data],
            )
        }
        IntegrationConfig::LzBridge(c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint,
                &pinocchio_token::ID.into(),
            );
            let authority_token_account = get_associated_token_address_with_program_id(
                &authority.pubkey(),
                &c.mint,
                &pinocchio_token::ID.into(),
            );
            let amount = match push_args {
                PushArgs::LzBridge { amount } => *amount,
                _ => panic!("No push args"),
            };

            let oft302: Oft302 = Oft302::new(c.program, DEVNET_RPC.to_owned());
            let quote_accounts = Oft302Accounts {
                // dummy payer for devnet fetch
                payer: pubkey!("Fty7h4FYAN7z8yjqaJExMHXbUoJYMcRjWYmggSxLbHp8"),
                token_mint: c.mint,
                token_escrow: LZ_USDS_ESCROW,
                peer_address: None,
            };
            let quote_params = Oft302QuoteParams {
                dst_eid: c.destination_eid,
                to: c.destination_address.to_bytes(),
                amount_ld: amount,
                min_amount_ld: amount,
            };
            let quote = oft302
                .quote(
                    quote_accounts.clone(),
                    quote_params.clone(),
                    Oft302Programs { endpoint: None },
                    vec![],
                )
                .await
                .unwrap();

            let send_accs = Oft302SendAccounts {
                payer: authority.pubkey(),
                token_mint: c.mint,
                token_escrow: LZ_USDS_ESCROW,
                token_source: authority_token_account,
                peer_address: None,
            };
            let send_params = SendInstructionArgs {
                dst_eid: c.destination_eid,
                to: c.destination_address.to_bytes(),
                amount_ld: quote_params.amount_ld,
                min_amount_ld: quote_params.min_amount_ld,
                options: vec![],
                compose_msg: None,
                native_fee: quote.native_fee,
                lz_token_fee: quote.lz_token_fee,
            };
            let send_programs = Oft302SendPrograms {
                endpoint: Some(LZ_ENDPOINT_PROGRAM_ID),
                token: Some(pinocchio_token::ID.into()),
            };
            let send_ix = oft302
                .send(send_accs, send_params, send_programs, vec![])
                .await?;

            // Load required Layer Zero accounts from devnet into litesvm environment.
            let rpc = RpcClient::new(DEVNET_RPC);
            for acc in send_ix.accounts.clone() {
                match rpc.get_account(&acc.pubkey) {
                    Ok(account) => {
                        if !account.executable {
                            svm.set_account(acc.pubkey, account)?
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch account {}: {:?}", acc.pubkey, e);
                    }
                }
            }
            post_ixns.push(send_ix.clone());

            (
                reserve_pda,
                reserve_pda, // repeat since only one required
                &[
                    AccountMeta {
                        pubkey: c.mint,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: vault,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: authority_token_account,
                        is_signer: false,
                        is_writable: true,
                    },
                    AccountMeta {
                        pubkey: pinocchio_token::ID.into(),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: pinocchio_associated_token_account::ID.into(),
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: system_program::ID,
                        is_signer: false,
                        is_writable: false,
                    },
                    AccountMeta {
                        pubkey: sysvar::instructions::ID,
                        is_signer: false,
                        is_writable: false,
                    },
                ],
                &[],
            )
        }
        _ => panic!("Invalid config for this type of PushArgs"),
    };

    let cu_limit_ixn = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
    let cu_price_ixn = ComputeBudgetInstruction::set_compute_unit_price(1);

    let main_ixn = PushBuilder::new()
        .push_args(push_args.clone())
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .integration(*integration)
        .reserve_a(reserve_a_pk)
        .reserve_b(reserve_b_pk)
        .add_remaining_accounts(remaining_accounts)
        .instruction();

    let mut signers: Vec<&Keypair> = vec![];
    signers.extend([authority]);
    signers.extend(additional_signers.iter());

    let mut ixns = vec![cu_limit_ixn, cu_price_ixn, main_ixn];
    if !post_ixns.is_empty() {
        ixns.append(&mut post_ixns);
    }
    let txn = Transaction::new_signed_with_payer(
        &ixns.to_vec(),
        Some(&authority.pubkey()),
        &signers,
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn);
    if tx_result.is_err() {
        println!("{:#?}", tx_result.clone().unwrap().logs);
    } else {
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    let integration_after = fetch_integration_account(svm, &integration)
        .expect("Failed to fetch reserve account")
        .unwrap();

    // Checks afterwards
    match &integration_account.config {
        IntegrationConfig::SplTokenExternal(ref c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault =
                get_associated_token_address_with_program_id(&controller_authority, &c.mint, &c.program);
            let reserve_a_after =
                fetch_reserve_account(svm, &reserve_pda).expect("Failed to fetch reserve account");
            let vault_a_balance_after = get_token_balance_or_zero(svm, &vault);
            let other_value_after = get_token_balance_or_zero(svm, &c.token_account);
            let vault_a_delta = vault_a_balance_before
                .checked_sub(vault_a_balance_after)
                .unwrap();
            println!("{:?}", reserve_a_after);
            println!("{:?}", integration_after);
            assert_ne!(
                vault_a_balance_before, vault_a_balance_after,
                "Vault A balance should have changed"
            );
            assert_ne!(
                other_value_before, other_value_after,
                "Other vault balance should have changed"
            );
        }
        IntegrationConfig::SplTokenSwap(ref c) => {
            let reserve_a_pda = derive_reserve_pda(controller, &c.mint_a);
            let reserve_b_pda = derive_reserve_pda(controller, &c.mint_b);
            let token_program_a = Pubkey::from(pinocchio_token::ID);
            let token_program_b = Pubkey::from(pinocchio_token::ID);
            let vault_a = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_a,
                &token_program_a,
            );
            let vault_b = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_b,
                &token_program_b,
            );
            let reserve_a_after = fetch_reserve_account(svm, &reserve_a_pda)
                .expect("Failed to fetch reserve account");
            let reserve_b_after = fetch_reserve_account(svm, &reserve_b_pda)
                .expect("Failed to fetch reserve account");
            let vault_a_balance_after = get_token_balance_or_zero(svm, &vault_a);
            let vault_b_balance_after = get_token_balance_or_zero(svm, &vault_b);
            let other_value_after = get_token_balance_or_zero(svm, &c.lp_token_account);
            println!("{:?}", reserve_a_after);
            println!("{:?}", reserve_b_after);
            println!("{:?}", other_value_after);
            println!("{:?}", integration_after);
        }
        IntegrationConfig::CctpBridge(ref c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint,
                &pinocchio_token::ID.into(),
            );
            let reserve_a_after = fetch_reserve_account(svm, &reserve_pda)
                .expect("Failed to fetch reserve account")
                .unwrap();
            let vault_a_balance_after = get_token_balance_or_zero(svm, &vault);
            let vault_a_delta = vault_a_balance_before
                .checked_sub(vault_a_balance_after)
                .unwrap();
            let other_value_after = get_mint_supply_or_zero(svm, &c.mint);
            let other_vault_delta = other_value_before.checked_sub(other_value_after).unwrap();
            let expected_amount = match push_args {
                PushArgs::CctpBridge { amount } => *amount,
                _ => panic!("Invalid type"),
            };
            println!("{:?}", integration_after);
            println!("{:?}", other_vault_delta);
            assert_eq!(
                vault_a_delta, expected_amount,
                "Vault balance should have reduced by the amount"
            );
            assert_eq!(
                other_vault_delta, expected_amount,
                "Mint supply should have reduced by the amount"
            );
        }
        IntegrationConfig::LzBridge(ref c) => {
            let reserve_pda = derive_reserve_pda(controller, &c.mint);
            let vault = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint,
                &pinocchio_token::ID.into(),
            );
            let reserve_a_after = fetch_reserve_account(svm, &reserve_pda)
                .expect("Failed to fetch reserve account")
                .unwrap();
            let vault_a_balance_after = get_token_balance_or_zero(svm, &vault);
            let vault_a_delta = vault_a_balance_before
                .checked_sub(vault_a_balance_after)
                .unwrap();
            let other_value_after = get_mint_supply_or_zero(svm, &c.mint);
            let other_vault_delta = other_value_before.checked_sub(other_value_after).unwrap();
            let expected_amount = match push_args {
                PushArgs::LzBridge { amount } => *amount,
                _ => panic!("Invalid type"),
            };
            println!("{:?}", integration_after);
            println!("{:?}", other_vault_delta);
            assert_eq!(
                vault_a_delta, expected_amount,
                "Vault balance should have reduced by the amount"
            );
            assert_eq!(
                other_vault_delta, expected_amount,
                "Mint supply should have reduced by the amount"
            );
        }
        _ => panic!("Not configured"),
    };

    Ok(tx_result)
}

pub fn pull_integration(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    integration: &Pubkey,
    authority: &Keypair,
    pull_args: &PullArgs,
) -> Result<(), Box<dyn Error>> {
    let calling_permission_pda = derive_permission_pda(controller, &authority.pubkey());
    let controller_authority  = derive_controller_authority_pda(controller);

    let integration_account = fetch_integration_account(svm, integration)
        .expect("Failed to fetch integration account")
        .unwrap();

    // To support checks after
    let integration_before = fetch_integration_account(svm, &integration)
        .expect("Failed to fetch reserve account")
        .unwrap();
    let mut reserve_a_before: Option<Reserve> = None;
    let mut reserve_b_before: Option<Reserve> = None;
    let mut vault_a_balance_before = 0u64;
    let mut vault_b_balance_before = 0u64;
    let mut other_value_before = 0u64;
    match &integration_account.config {
        IntegrationConfig::SplTokenSwap(ref c) => {
            let reserve_a_pda = derive_reserve_pda(controller, &c.mint_a);
            let reserve_b_pda = derive_reserve_pda(controller, &c.mint_b);
            let token_program_a = Pubkey::from(pinocchio_token::ID);
            let token_program_b = Pubkey::from(pinocchio_token::ID);
            let vault_a = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_a,
                &token_program_a,
            );
            let vault_b = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_b,
                &token_program_b,
            );
            reserve_a_before = fetch_reserve_account(svm, &reserve_a_pda)
                .expect("Failed to fetch reserve account");
            reserve_b_before = fetch_reserve_account(svm, &reserve_b_pda)
                .expect("Failed to fetch reserve account");
            vault_a_balance_before = get_token_balance_or_zero(svm, &vault_a);
            vault_b_balance_before = get_token_balance_or_zero(svm, &vault_b);
            other_value_before = get_token_balance_or_zero(svm, &c.lp_token_account);
            println!("{:?}", reserve_a_before);
            println!("{:?}", reserve_b_before);
            println!("{:?}", other_value_before);
            println!("{:?}", integration_before);
        }
        _ => panic!("Not configured"),
    };

    let (reserve_a_pk, reserve_b_pk, remaining_accounts): (Pubkey, Pubkey, &[AccountMeta]) =
        match &integration_account.config {
            IntegrationConfig::SplTokenSwap(ref c) => {
                let reserve_a_pda = derive_reserve_pda(controller, &c.mint_a);
                let reserve_b_pda = derive_reserve_pda(controller, &c.mint_b);
                let token_program_a = Pubkey::from(pinocchio_token::ID);
                let token_program_b = Pubkey::from(pinocchio_token::ID);
                let token_program_lp = Pubkey::from(pinocchio_token::ID);
                let vault_a = get_associated_token_address_with_program_id(
                    &controller_authority,
                    &c.mint_a,
                    &token_program_a,
                );
                let vault_b = get_associated_token_address_with_program_id(
                    &controller_authority,
                    &c.mint_b,
                    &token_program_b,
                );
                let vault_lp = get_associated_token_address_with_program_id(
                    &controller_authority,
                    &c.lp_mint,
                    &token_program_lp,
                );
                let (swap_authority, _) = derive_swap_authority_pda_and_bump(&c.swap, &c.program);
                let swap_token_a = get_associated_token_address_with_program_id(
                    &swap_authority,
                    &c.mint_a,
                    &token_program_a,
                );
                let swap_token_b = get_associated_token_address_with_program_id(
                    &swap_authority,
                    &c.mint_b,
                    &token_program_b,
                );
                let swap_fee_account = get_associated_token_address_with_program_id(
                    &NOVA_TOKEN_SWAP_FEE_OWNER,
                    &c.lp_mint,
                    &token_program_lp,
                );
                (
                    reserve_a_pda,
                    reserve_b_pda,
                    &[
                        AccountMeta {
                            pubkey: c.swap,
                            is_signer: false,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: c.mint_a,
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: c.mint_b,
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: c.lp_mint,
                            is_signer: false,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: c.lp_token_account,
                            is_signer: false,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: token_program_a,
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: token_program_b,
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: token_program_lp,
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: swap_token_a,
                            is_signer: false,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: swap_token_b,
                            is_signer: false,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: vault_a,
                            is_signer: false,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: vault_b,
                            is_signer: false,
                            is_writable: true,
                        },
                        AccountMeta {
                            pubkey: c.program,
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: pinocchio_associated_token_account::ID.into(),
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: swap_authority,
                            is_signer: false,
                            is_writable: false,
                        },
                        AccountMeta {
                            pubkey: swap_fee_account,
                            is_signer: false,
                            is_writable: true,
                        },
                    ],
                )
            }
            _ => panic!("Invalid config for this type of PushArgs"),
        };

    let cu_limit_ixn = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
    let cu_price_ixn = ComputeBudgetInstruction::set_compute_unit_price(1);

    let main_ixn = PullBuilder::new()
        .pull_args(pull_args.clone())
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .integration(*integration)
        .reserve_a(reserve_a_pk)
        .reserve_b(reserve_b_pk)
        .add_remaining_accounts(remaining_accounts)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[cu_limit_ixn, cu_price_ixn, main_ixn],
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

    let integration_after = fetch_integration_account(svm, &integration)
        .expect("Failed to fetch reserve account")
        .unwrap();

    // Checks afterwards
    match &integration_account.config {
        IntegrationConfig::SplTokenSwap(ref c) => {
            let reserve_a_pda = derive_reserve_pda(controller, &c.mint_a);
            let reserve_b_pda = derive_reserve_pda(controller, &c.mint_b);
            let token_program_a = Pubkey::from(pinocchio_token::ID);
            let token_program_b = Pubkey::from(pinocchio_token::ID);
            let vault_a = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_a,
                &token_program_a,
            );
            let vault_b = get_associated_token_address_with_program_id(
                &controller_authority,
                &c.mint_b,
                &token_program_b,
            );
            let reserve_a_after = fetch_reserve_account(svm, &reserve_a_pda)
                .expect("Failed to fetch reserve account");
            let reserve_b_after = fetch_reserve_account(svm, &reserve_b_pda)
                .expect("Failed to fetch reserve account");
            let vault_a_balance_after = get_token_balance_or_zero(svm, &vault_a);
            let vault_b_balance_after = get_token_balance_or_zero(svm, &vault_b);
            let other_value_after = get_token_balance_or_zero(svm, &c.lp_token_account);
            println!("{:?}", reserve_a_after);
            println!("{:?}", reserve_b_after);
            println!("{:?}", other_value_after);
            println!("{:?}", integration_after);
        }
        _ => panic!("Not configured"),
    };

    Ok(())
}
