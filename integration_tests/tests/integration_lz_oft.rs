mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, controller::manage_controller, initialize_ata, initialize_contoller,
    initialize_reserve, manage_permission,
};
use crate::{
    helpers::constants::USDS_TOKEN_MINT_PUBKEY,
    subs::{edit_ata_amount, transfer_tokens},
};
use borsh::BorshDeserialize;
use endpoint_client::types::MessagingReceipt;
use helpers::lite_svm_with_programs;
use solana_program::pubkey;
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::ReserveStatus;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};

#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use oft_client::{
        instructions::SendInstructionArgs,
        oft302::{
            Oft302, Oft302Accounts, Oft302Programs, Oft302QuoteParams, Oft302SendAccounts,
            Oft302SendPrograms,
        },
    };
    use solana_client::rpc_client::RpcClient;
    use solana_sdk::{
        clock::Clock,
        compute_budget::ComputeBudgetInstruction,
        instruction::{Instruction, InstructionError},
        pubkey::Pubkey,
        transaction::{Transaction, TransactionError},
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        create_lz_bridge_initialize_integration_instruction, create_manage_integration_instruction,
        create_sync_integration_instruction,
        generated::instructions::ResetLzPushInFlightBuilder,
    };
    use test_case::test_case;

    use crate::{
        helpers::{
            assert::assert_custom_error,
            cctp::evm_address_to_solana_pubkey,
            constants::{
                DEVNET_RPC, LZ_DESTINATION_DOMAIN_EID, LZ_ENDPOINT_PROGRAM_ID, LZ_USDS_ESCROW,
                LZ_USDS_OFT_PROGRAM_ID, LZ_USDS_OFT_STORE_PUBKEY, LZ_USDS_PEER_CONFIG_PUBKEY,
            },
            lite_svm::get_account_data_from_json,
            lz_oft::create_lz_push_and_send_ixs,
            utils::get_program_return_data,
        },
        subs::{
            derive_controller_authority_pda, fetch_integration_account,
            fetch_reserve_account, get_token_balance_or_zero, ReserveKeys,
        },
    };

    use super::*;

    const EVM_DESTINATION: &str = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";

    fn setup_env_sans_integration(
        svm: &mut LiteSVM,
    ) -> Result<(Pubkey, Keypair, ReserveKeys), Box<dyn std::error::Error>> {
        let authority: Keypair = Keypair::new();
        // Load LZ OFT specific accounts.
        // Note: These are arbitrary accounts necessary for the OFT Send instruction.
        // These are not named as they have not been matched up with their corresponding
        // index on the OFT Send IX.
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_2uk9pQh3tB5ErV7LGQJcbWjb4KeJ2UJki5qJZ8QG56G3.json");
        svm.set_account(
            pubkey!("2uk9pQh3tB5ErV7LGQJcbWjb4KeJ2UJki5qJZ8QG56G3"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_2XgGZG4oP29U3w5h4nTk1V2LFHL23zKDPJjs3psGzLKQ.json");
        svm.set_account(
            pubkey!("2XgGZG4oP29U3w5h4nTk1V2LFHL23zKDPJjs3psGzLKQ"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_4VDjp6XQaxoZf5RGwiPU9NR1EXSZn2TP4ATMmiSzLfhb.json");
        svm.set_account(
            pubkey!("4VDjp6XQaxoZf5RGwiPU9NR1EXSZn2TP4ATMmiSzLfhb"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_6JVxntrMiSckkojEiPk4pNMkVDVfAicjZKWNxzf56UmY.json");
        svm.set_account(
            pubkey!("6JVxntrMiSckkojEiPk4pNMkVDVfAicjZKWNxzf56UmY"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_8Kx7Q7vredpvHaK7a3NEDdveQrqnpwUSfZurxTXAaEqH.json");
        svm.set_account(
            pubkey!("8Kx7Q7vredpvHaK7a3NEDdveQrqnpwUSfZurxTXAaEqH"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_526PeNZfw8kSnDU4nmzJFVJzJWNhwmZykEyJr5XWz5Fv.json");
        svm.set_account(
            pubkey!("526PeNZfw8kSnDU4nmzJFVJzJWNhwmZykEyJr5XWz5Fv"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_911rFremHQR6Z9pPJVNchkg5GmZzysbsg3hk9NppooPM.json");
        svm.set_account(
            pubkey!("911rFremHQR6Z9pPJVNchkg5GmZzysbsg3hk9NppooPM"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_AnF6jGBQykDchX1EjmQePJwJBCh9DSbZjYi14Hdx5BRx.json");
        svm.set_account(
            pubkey!("AnF6jGBQykDchX1EjmQePJwJBCh9DSbZjYi14Hdx5BRx"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_AwrbHeCyniXaQhiJZkLhgWdUCteeWSGaSN1sTfLiY7xK.json");
        svm.set_account(
            pubkey!("AwrbHeCyniXaQhiJZkLhgWdUCteeWSGaSN1sTfLiY7xK"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_CSFsUupvJEQQd1F4SsXGACJaxQX4eropQMkGV2696eeQ.json");
        svm.set_account(
            pubkey!("CSFsUupvJEQQd1F4SsXGACJaxQX4eropQMkGV2696eeQ"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_D6vis7fffY53WCXL7EZPLbsLKhLgmg16PyDXytShypfz.json");
        svm.set_account(
            pubkey!("D6vis7fffY53WCXL7EZPLbsLKhLgmg16PyDXytShypfz"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();
        let lz_usds_oft_devnet_acct = get_account_data_from_json("./fixtures/lz_oft_devnet/lz_oft_devnet_HwpzV5qt9QzYRuWkHqTRuhbqtaMhapSNuriS5oMynkny.json");
        svm.set_account(
            pubkey!("HwpzV5qt9QzYRuWkHqTRuhbqtaMhapSNuriS5oMynkny"),
            lz_usds_oft_devnet_acct,
        )
        .unwrap();

        // Airdrop to payer
        airdrop_lamports(svm, &authority.pubkey(), 1_000_000_000)?;

        // Create an ATA for the USDC account
        let _authority_usds_ata = initialize_ata(
            svm,
            &authority,
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            svm,
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Update the authority to have all permissions
        let _ = manage_permission(
            svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &authority.pubkey(), // subject authority
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

        // Initialize a reserve for the token
        let usds_reserve_keys = initialize_reserve(
            svm,
            &controller_pk,
            &USDS_TOKEN_MINT_PUBKEY, // mint
            &authority,              // payer
            &authority,              // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            svm,
            &authority,
            &authority,
            &USDS_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        Ok((controller_pk, authority, usds_reserve_keys))
    }

    fn setup_env(
        svm: &mut LiteSVM,
        permit_liquidation: bool,
    ) -> Result<(Pubkey, Pubkey, Keypair, ReserveKeys), Box<dyn std::error::Error>> {
        let (controller_pk, authority, usds_reserve_keys) = setup_env_sans_integration(svm)?;

        // Serialize the destination address appropriately
        let evm_address = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        // Initialize an integration
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let init_ix = create_lz_bridge_initialize_integration_instruction(
            &authority.pubkey(),
            &controller_pk,
            &authority.pubkey(),
            "ETH USDS LZ Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &LZ_USDS_OFT_PROGRAM_ID,
            &LZ_USDS_ESCROW,
            &destination_address,
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
        );

        // Integration is at index 5 in the IX
        let integration_pubkey = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        Ok((
            controller_pk,
            integration_pubkey,
            authority,
            usds_reserve_keys,
        ))
    }

    fn create_lz_push_ix(
        controller: &Pubkey,
        integration: &Pubkey,
        authority: &Keypair,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        let reserve_pda = svm_alm_controller_client::derive_reserve_pda(&controller, &USDS_TOKEN_MINT_PUBKEY);

        let amount = 2000;
        let push_ix = svm_alm_controller_client::create_lz_bridge_push_instruction(
            controller,
            &authority.pubkey(),
            integration,
            &reserve_pda,
            &spl_token::ID,
            &USDS_TOKEN_MINT_PUBKEY,
            amount,
        );

        Ok(push_ix)
    }

    /// Deprecated
    async fn create_oft_send_ix(
        svm: &mut LiteSVM,
        authority: &Keypair,
        token_source: Pubkey,
        amount: u64,
        skip_setup: bool,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        // Serialize the destination address appropriately
        let evm_address = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let oft302 = Oft302::new(LZ_USDS_OFT_PROGRAM_ID, DEVNET_RPC.to_owned());
        let (native_fee, lz_token_fee) = if !skip_setup {
            let quote_accounts = Oft302Accounts {
                // dummy payer for devnet fetch
                payer: pubkey!("Fty7h4FYAN7z8yjqaJExMHXbUoJYMcRjWYmggSxLbHp8"),
                token_mint: USDS_TOKEN_MINT_PUBKEY,
                token_escrow: LZ_USDS_ESCROW,
                peer_address: None,
            };
            let quote_params = Oft302QuoteParams {
                dst_eid: LZ_DESTINATION_DOMAIN_EID,
                to: destination_address.to_bytes(),
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
            (quote.native_fee, quote.lz_token_fee)
        } else {
            (0, 0)
        };

        let send_accs = Oft302SendAccounts {
            payer: authority.pubkey(),
            token_mint: USDS_TOKEN_MINT_PUBKEY,
            token_escrow: LZ_USDS_ESCROW,
            token_source,
            peer_address: None,
        };
        let send_params = SendInstructionArgs {
            dst_eid: LZ_DESTINATION_DOMAIN_EID,
            to: destination_address.to_bytes(),
            amount_ld: amount,
            min_amount_ld: amount,
            options: vec![],
            compose_msg: None,
            native_fee: native_fee,
            lz_token_fee: lz_token_fee,
        };
        let send_programs = Oft302SendPrograms {
            endpoint: Some(LZ_ENDPOINT_PROGRAM_ID),
            token: Some(pinocchio_token::ID.into()),
        };
        let send_ixn = oft302
            .send(send_accs, send_params, send_programs, vec![])
            .await?;

        // Load required Layer Zero accounts from devnet into litesvm environment.
        if !skip_setup {
            let rpc = RpcClient::new(DEVNET_RPC);
            for acc in send_ixn.accounts.clone() {
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
        }
        Ok(send_ixn)
    }

    #[test]
    fn lz_bridge_init_success() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let (controller_pk, authority, _reserve_keys) = setup_env_sans_integration(&mut svm)?;

        // Serialize the destination address appropriately
        let evm_address = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_lz_bridge_initialize_integration_instruction(
            &authority.pubkey(),
            &controller_pk,
            &authority.pubkey(),
            "ETH USDS LZ Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &LZ_USDS_OFT_PROGRAM_ID,
            &LZ_USDS_ESCROW,
            &destination_address,
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
        );

        // Integration is at index 5 in the IX
        let integration_pubkey = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let integration = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        let clock = svm.get_sysvar::<Clock>();

        assert_eq!(integration.controller, controller_pk);
        assert_eq!(integration.status, IntegrationStatus::Active);
        assert_eq!(integration.rate_limit_slope, rate_limit_slope);
        assert_eq!(integration.rate_limit_max_outflow, rate_limit_max_outflow);
        assert_eq!(
            integration.rate_limit_outflow_amount_available,
            rate_limit_max_outflow
        );
        assert_eq!(integration.rate_limit_remainder, 0);
        assert_eq!(integration.permit_liquidation, permit_liquidation);
        assert_eq!(integration.last_refresh_timestamp, clock.unix_timestamp);
        assert_eq!(integration.last_refresh_slot, clock.slot);

        match integration.config {
            IntegrationConfig::LzBridge(c) => {
                assert_eq!(c.destination_address, destination_address);
                assert_eq!(c.destination_eid, LZ_DESTINATION_DOMAIN_EID);
                assert_eq!(c.mint, USDS_TOKEN_MINT_PUBKEY);
                assert_eq!(c.oft_store, LZ_USDS_OFT_STORE_PUBKEY);
                assert_eq!(c.oft_token_escrow, LZ_USDS_ESCROW);
                assert_eq!(c.peer_config, LZ_USDS_PEER_CONFIG_PUBKEY);
                assert_eq!(c.program, LZ_USDS_OFT_PROGRAM_ID);
            }
            _ => panic!("invalid config"),
        };
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lz_push_with_oft_send_success() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller_pk, lz_usds_eth_bridge_integration_pk, authority, reserve_keys) =
            setup_env(&mut svm, false)?;

        let integration_before =
            fetch_integration_account(&svm, &lz_usds_eth_bridge_integration_pk)
                .unwrap()
                .unwrap();
        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let balance_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        // Push the integration -- i.e. bridge using LZ OFT
        let amount = 2000;
        let ixs = create_lz_push_and_send_ixs(
            &controller_pk,
            &authority.pubkey(),
            &lz_usds_eth_bridge_integration_pk,
            &reserve_keys.pubkey,
            &LZ_USDS_OFT_PROGRAM_ID,
            &spl_token::ID,
            &evm_address_to_solana_pubkey(EVM_DESTINATION),
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
            amount,
        )
        .await?;
        let result = svm
            .send_transaction(Transaction::new_signed_with_payer(
                &ixs,
                Some(&authority.pubkey()),
                &[&authority],
                svm.latest_blockhash(),
            ))
            .map_err(|e| e.err.to_string())?;

        let integration_after = fetch_integration_account(&svm, &lz_usds_eth_bridge_integration_pk)
            .unwrap()
            .unwrap();
        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        // Assert Integration rate limits adjusted
        let integration_rate_limit_delta = integration_before.rate_limit_outflow_amount_available
            - integration_after.rate_limit_outflow_amount_available;
        assert_eq!(integration_rate_limit_delta, amount);
        // Assert Reserve rate limits adjusted
        let reserve_rate_limit_delta = reserve_before.rate_limit_outflow_amount_available
            - reserve_after.rate_limit_outflow_amount_available;
        assert_eq!(reserve_rate_limit_delta, amount);
        // Assert Reserve vault was debited exact amount
        let vault_balance_delta = balance_before - balance_after;
        assert_eq!(vault_balance_delta, amount);

        // Check that OFT return data exists and amount matches.
        let return_data = get_program_return_data(result.logs, &LZ_USDS_OFT_PROGRAM_ID).unwrap();
        let (_messaging_receipt, oft_receipt) =
            <(MessagingReceipt, oft_client::types::OFTReceipt)>::try_from_slice(&return_data)
                .map_err(|err| format!("Failed to parse result: {}", err))
                .unwrap();
        assert_eq!(oft_receipt.amount_sent_ld, amount);
        assert_eq!(oft_receipt.amount_received_ld, amount);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lz_push_tx_introspection_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller, integration, authority, _reserve_keys) = setup_env(&mut svm, false)?;

        let authority_token_account = get_associated_token_address_with_program_id(
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            &pinocchio_token::ID.into(),
        );
        let amount = 2000;

        let lz_push_ix = create_lz_push_ix(&controller, &integration, &authority)?;
        let send_ixn =
            create_oft_send_ix(&mut svm, &authority, authority_token_account, amount, true).await?;
        let reset_ix = ResetLzPushInFlightBuilder::new()
            .controller(controller)
            .integration(integration)
            .instruction();
        let cu_limit_ixn: Instruction = ComputeBudgetInstruction::set_compute_unit_limit(400_000);

        // Expect failure with not enough IXs
        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure without send_ixn.
        let txn = Transaction::new_signed_with_payer(
            &[cu_limit_ixn.clone(), lz_push_ix.clone(), reset_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(
            &tx_result,
            1,
            SvmAlmControllerErrors::InvalidInstructionIndex,
        );

        // Expect failure without reset_ixn as last.
        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ix.clone(), send_ixn, cu_limit_ixn.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when token_escrow doesn't match.
        let send_ixn =
            create_oft_send_ix(&mut svm, &authority, Pubkey::new_unique(), amount, true).await?;
        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ix.clone(), send_ixn, reset_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when send amount doesn't match.
        let send_ixn =
            create_oft_send_ix(&mut svm, &authority, authority_token_account, 111, true).await?;
        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ix.clone(), send_ixn, reset_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiple_push_with_single_send_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller, integration, authority, _reserve_keys) = setup_env(&mut svm, false)?;

        let authority_token_account = get_associated_token_address_with_program_id(
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            &pinocchio_token::ID.into(),
        );
        let amount = 2000;

        let lz_push_ixn = create_lz_push_ix(&controller, &integration, &authority)?;
        let send_ixn =
            create_oft_send_ix(&mut svm, &authority, authority_token_account, amount, false)
                .await?;
        let reset_ix = ResetLzPushInFlightBuilder::new()
            .controller(controller)
            .integration(integration)
            .instruction();

        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ixn.clone(), lz_push_ixn.clone(), send_ixn, reset_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidInstructionIndex,
        );
        Ok(())
    }

    #[test_case(true, false, false, false, false, false, false, false, false, false, false; "can_manage_permissions fails")]
    #[test_case(false, true, false, false, false, false, false, false, false, false, false; "can_invoke_external_transfer fails")]
    #[test_case(false, false, true, false, false, false, false, false, false, false, false; "can_execute_swap fails")]
    #[test_case(false, false, false, true, false, false, false, false, false, false, true; "can_reallocate passes")]
    #[test_case(false, false, false, false, true, false, false, false, false, false, false; "can_freeze_controller fails")]
    #[test_case(false, false, false, false, false, true, false, false, false, false, false; "can_unfreeze_controller fails")]
    #[test_case(false, false, false, false, false, false, true, false, false, false, false; "can_manage_reserves_and_integrations fails")]
    #[test_case(false, false, false, false, false, false, false, true, false, false, false; "can_suspend_permissions fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, false, false; "can_liquidate w/o permit_liquidation fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, true, true; "can_liquidate w/ permit_liquidation passes")]
    #[tokio::test(flavor = "multi_thread")]
    async fn lz_oft_permissions(
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_execute_swap: bool,
        can_reallocate: bool,
        can_freeze_controller: bool,
        can_unfreeze_controller: bool,
        can_manage_reserves_and_integrations: bool,
        can_suspend_permissions: bool,
        can_liquidate: bool,
        permit_liquidation: bool,
        result_ok: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller_pk, lz_usds_eth_bridge_integration_pk, super_authority, reserve_keys) =
            setup_env(&mut svm, permit_liquidation)?;

        let push_authority = Keypair::new();
        airdrop_lamports(&mut svm, &push_authority.pubkey(), 1_000_000_000)?;
        // Update the authority to have permissions
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,         // payer
            &super_authority,         // calling authority
            &push_authority.pubkey(), // subject authority
            PermissionStatus::Active,
            can_execute_swap,                     // can_execute_swap,
            can_manage_permissions,               // can_manage_permissions,
            can_invoke_external_transfer,         // can_invoke_external_transfer,
            can_reallocate,                       // can_reallocate,
            can_freeze_controller,                // can_freeze,
            can_unfreeze_controller,              // can_unfreeze,
            can_manage_reserves_and_integrations, // can_manage_reserves_and_integrations
            can_suspend_permissions,              // can_suspend_permissions
            can_liquidate,                        // can_liquidate
        )?;

        // Push the integration -- i.e. bridge using LZ OFT
        let amount = 2000;
        let ixs = create_lz_push_and_send_ixs(
            &controller_pk,
            &push_authority.pubkey(),
            &lz_usds_eth_bridge_integration_pk,
            &reserve_keys.pubkey,
            &LZ_USDS_OFT_PROGRAM_ID,
            &spl_token::ID,
            &evm_address_to_solana_pubkey(EVM_DESTINATION),
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
            amount,
        )
        .await?;
        let tx_res = svm.send_transaction(Transaction::new_signed_with_payer(
            &ixs,
            Some(&push_authority.pubkey()),
            &[&push_authority],
            svm.latest_blockhash(),
        ));

        // Assert the expected result given the enabled privilege
        match result_ok {
            true => assert!(tx_res.is_ok()),
            false => assert_eq!(
                tx_res.err().unwrap().err,
                TransactionError::InstructionError(0, InstructionError::IncorrectAuthority)
            ),
        }

        Ok(())
    }

    #[test]
    fn test_lz_oft_init_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let (controller_pk, authority, _reserve_keys) = setup_env_sans_integration(&mut svm)?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let init_ix = create_lz_bridge_initialize_integration_instruction(
            &authority.pubkey(),
            &controller_pk,
            &authority.pubkey(),
            "ETH USDS LZ Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            false,
            &LZ_USDS_OFT_PROGRAM_ID,
            &LZ_USDS_ESCROW,
            &destination_address,
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
        );

        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_res = svm.send_transaction(tx);

        assert_custom_error(&tx_res, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_lz_oft_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller_pk, lz_usds_eth_bridge_integration_pk, super_authority, _reserve_keys) =
            setup_env(&mut svm, false)?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Try to push the integration -- i.e. bridge using LZ OFT
        let amount = 2000;
        let ixs = create_lz_push_and_send_ixs(
            &controller_pk,
            &super_authority.pubkey(),
            &lz_usds_eth_bridge_integration_pk,
            &svm_alm_controller_client::derive_reserve_pda(&controller_pk, &USDS_TOKEN_MINT_PUBKEY),
            &LZ_USDS_OFT_PROGRAM_ID,
            &spl_token::ID,
            &evm_address_to_solana_pubkey(EVM_DESTINATION),
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
            amount,
        )
        .await?;
        let tx_res = svm.send_transaction(Transaction::new_signed_with_payer(
            &ixs,
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(
            &tx_res,
            0,
            SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction,
        );

        Ok(())
    }
}
