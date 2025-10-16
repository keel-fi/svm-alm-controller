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
    use solana_sdk::{
        account::Account,
        clock::Clock,
        compute_budget::ComputeBudgetInstruction,
        instruction::{Instruction, InstructionError},
        pubkey::Pubkey,
        transaction::{Transaction, TransactionError},
    };
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        create_lz_bridge_initialize_integration_instruction,
        generated::types::{
            AccountingAction, AccountingDirection, AccountingEvent, IntegrationUpdateEvent,
            SvmAlmControllerEvent,
        },
    };
    use test_case::test_case;

    use crate::{
        helpers::{
            assert::assert_custom_error,
            cctp::evm_address_to_solana_pubkey,
            constants::{
                LZ_DESTINATION_DOMAIN_EID, LZ_USDS_ESCROW, LZ_USDS_OFT_PROGRAM_ID,
                LZ_USDS_OFT_STORE_PUBKEY, LZ_USDS_PEER_CONFIG_PUBKEY,
            },
            lite_svm::get_account_data_from_json,
            lz_oft::create_lz_push_and_send_ixs,
            utils::get_program_return_data,
        },
        subs::{
            derive_controller_authority_pda, fetch_integration_account, fetch_reserve_account,
            get_token_balance_or_zero, initialize_mint, ReserveKeys,
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
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
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
        let reserve_pda =
            svm_alm_controller_client::derive_reserve_pda(&controller, &USDS_TOKEN_MINT_PUBKEY);

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
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone())
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

        match integration.clone().config {
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

        // Assert event is emitted
        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: integration_pubkey,
            authority: authority.pubkey(),
            old_state: None,
            new_state: Some(integration),
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            expected_event
        );
        Ok(())
    }

    #[test]
    fn lz_bridge_init_fails_with_transfer_fee_mint() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let (controller_pk, authority, _reserve_keys) = setup_env_sans_integration(&mut svm)?;

        // create a Token2022 mint with transfer fee extension
        let transfer_fee_mint = initialize_mint(
            &mut svm,
            &authority,
            &authority.pubkey(),
            None,
            6,
            None,
            &spl_token_2022::ID,
            Some(10),
        )?;

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
            "ETH Transfer fee Mint LZ Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &LZ_USDS_OFT_PROGRAM_ID,
            &LZ_USDS_ESCROW,
            &destination_address,
            LZ_DESTINATION_DOMAIN_EID,
            &transfer_fee_mint,
        );

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ));

        // Assert InvalidTokenMintExtension error
        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidTokenMintExtension,
        );

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
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let result = svm
            .send_transaction(tx.clone())
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

        let check_delta = vault_balance_delta;
        // Assert accounting events
        let expected_debit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            reserve: Some(reserve_keys.pubkey),
            mint: USDS_TOKEN_MINT_PUBKEY,
            action: AccountingAction::BridgeSend,
            delta: check_delta,
            direction: AccountingDirection::Debit,
        });
        assert_contains_controller_cpi_event!(
            result,
            tx.message.account_keys.as_slice(),
            expected_debit_event
        );

        let expected_credit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(lz_usds_eth_bridge_integration_pk),
            reserve: None,
            mint: USDS_TOKEN_MINT_PUBKEY,
            action: AccountingAction::BridgeSend,
            delta: check_delta,
            direction: AccountingDirection::Credit,
        });
        assert_contains_controller_cpi_event!(
            result,
            tx.message.account_keys.as_slice(),
            expected_credit_event
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lz_push_tx_introspection_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller, integration, authority, reserve_keys) = setup_env(&mut svm, false)?;

        let amount = 2000;

        let [lz_push_ix, mut send_ixn, reset_ix] = create_lz_push_and_send_ixs(
            &controller,
            &authority.pubkey(),
            &integration,
            &reserve_keys.pubkey,
            &LZ_USDS_OFT_PROGRAM_ID,
            &spl_token::ID,
            &evm_address_to_solana_pubkey(EVM_DESTINATION),
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
            amount,
        )
        .await
        .unwrap();
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
            &[lz_push_ix.clone(), send_ixn.clone(), cu_limit_ixn.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when token_escrow doesn't match.
        let valid_token_escrow = send_ixn.accounts[4].pubkey;
        send_ixn.accounts[4].pubkey = Pubkey::new_unique();
        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ix.clone(), send_ixn.clone(), reset_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);
        send_ixn.accounts[4].pubkey = valid_token_escrow;

        // Expect failure when send amount doesn't match.
        let valid_send_data = send_ixn.data;
        let mut invalid_send_data = valid_send_data.clone();
        invalid_send_data[44..52].copy_from_slice(&111u64.to_le_bytes());
        send_ixn.data = invalid_send_data;
        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ix.clone(), send_ixn.clone(), reset_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);
        send_ixn.data = valid_send_data;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiple_push_with_single_send_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller, integration, authority, reserve_keys) = setup_env(&mut svm, false)?;

        let amount = 2000;

        let [lz_push_ixn, send_ixn, reset_ix] = create_lz_push_and_send_ixs(
            &controller,
            &authority.pubkey(),
            &integration,
            &reserve_keys.pubkey,
            &LZ_USDS_OFT_PROGRAM_ID,
            &spl_token::ID,
            &evm_address_to_solana_pubkey(EVM_DESTINATION),
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
            amount,
        )
        .await
        .unwrap();

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

    #[test]
    fn lz_push_with_invalid_controller_authority_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller, integration, authority, _reserve_keys) = setup_env(&mut svm, false)?;

        let mut lz_push_ixn = create_lz_push_ix(&controller, &integration, &authority)?;

        // Modify controller authority (index 1) to a different pubkey
        lz_push_ixn.accounts[1].pubkey = Pubkey::new_unique();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[lz_push_ixn],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidControllerAuthority,
        );

        Ok(())
    }

    #[test]
    fn lz_init_inner_ctx_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let (controller_pk, authority, _) = setup_env_sans_integration(&mut svm)?;

        // Serialize the destination address appropriately
        let evm_address = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        // Initialize an integration
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let mut init_ix = create_lz_bridge_initialize_integration_instruction(
            &authority.pubkey(),
            &controller_pk,
            &authority.pubkey(),
            "ETH USDS LZ Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            true,
            &LZ_USDS_OFT_PROGRAM_ID,
            &LZ_USDS_ESCROW,
            &destination_address,
            LZ_DESTINATION_DOMAIN_EID,
            &USDS_TOKEN_MINT_PUBKEY,
        );

        // Checks for inner_ctx accounts:
        // (index 8) mint:
        //      owner by token2022 or spl token
        // (index 9) oft_store:
        //      owned by inner_ctx.oft_program,
        //      (deserialized).token_mint == inner_ctx.mint,
        //      (deserialized).token_escrow == inner_ctx.token_escrow
        // (index 10) peer_config:
        //      owned by inner_ctx.oft_program,
        //      pubkey matches PDA,
        //      succesfull deserialization

        // modify oft_store.token_mint
        let oft_store_pk = init_ix.accounts[9].pubkey;
        let oft_store_acc_before = svm
            .get_account(&oft_store_pk)
            .expect("failed to fetch account");
        let oft_store_data_wrong_mint = Account {
            data: vec![
                oft_store_acc_before.data[..17].to_vec(),
                Pubkey::new_unique().to_bytes().to_vec(),
                oft_store_acc_before.data[49..].to_vec(),
            ]
            .concat(),
            ..oft_store_acc_before
        };
        svm.set_account(oft_store_pk, oft_store_data_wrong_mint)
            .expect("failed to set account");
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        svm.set_account(oft_store_pk, oft_store_acc_before.clone())
            .expect("failed to set account");
        svm.expire_blockhash();

        // modify oft_store.token_escrow
        let oft_store_data_wrong_escrow = Account {
            data: vec![
                oft_store_acc_before.data[..49].to_vec(),
                Pubkey::new_unique().to_bytes().to_vec(),
                oft_store_acc_before.data[81..].to_vec(),
            ]
            .concat(),
            ..oft_store_acc_before
        };
        svm.set_account(oft_store_pk, oft_store_data_wrong_escrow)
            .expect("failed to set account");
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        svm.set_account(oft_store_pk, oft_store_acc_before)
            .expect("failed to set account");
        svm.expire_blockhash();

        // modify peer_config pubkey
        let peer_config_pk_before = init_ix.accounts[10].pubkey;
        let peer_config_data_before = svm
            .get_account(&peer_config_pk_before)
            .expect("failed to get account");
        let peer_config_pk_after = Pubkey::new_unique();
        svm.set_account(peer_config_pk_after, peer_config_data_before.clone())
            .expect("failed to set account");
        init_ix.accounts[10].pubkey = peer_config_pk_after;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidPda);
        init_ix.accounts[10].pubkey = peer_config_pk_before;
        svm.expire_blockhash();

        // modify peer_config data so deserialization fails
        svm.set_account(
            peer_config_pk_before,
            Account {
                data: [1; 32].to_vec(),
                ..peer_config_data_before.clone()
            },
        )
        .expect("failed to set account");
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        svm.set_account(peer_config_pk_before, peer_config_data_before)
            .expect("failed to set account");
        svm.expire_blockhash();

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&authority)];
        test_invalid_accounts!(
            svm.clone(),
            authority.pubkey(),
            signers,
            init_ix.clone(),
            {
                // modify mint owner
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: Invalid owner"),
                // modify oft_store owner
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Oft Store: Invalid owner"),
                // modify peer_config owner
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Peer Config: Invalid owner"),
            }
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lz_push_inner_ctx_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller_pk, lz_usds_eth_bridge_integration_pk, authority, reserve_keys) =
            setup_env(&mut svm, false)?;

        // Push the integration -- i.e. bridge using LZ OFT
        let amount = 2000;
        let [mut push_ix, send_ix, reset_ix] = create_lz_push_and_send_ixs(
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

        // Checks for inner_ctx accounts:
        // (index 7) mint
        //      pubkey == config.mint
        //      pubkey == reserve_a.mint
        // (index 8) vault
        //      pubkey == reserve_a.vault
        // (index 10) token program
        //      pubkey == spl token or token2022
        // (index 11) associated token program
        //      pubkey == AT program
        // (index 12) system program
        //      pubkey == system program id
        // (index 13) sysvar instruction
        //      pubkey == sysvar instructions id

        // modify vault pubkey so it doesnt match integration config
        let vault_pk_before = push_ix.accounts[8].pubkey;
        push_ix.accounts[8].pubkey = Pubkey::new_unique();
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[push_ix.clone(), send_ix.clone(), reset_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        push_ix.accounts[8].pubkey = vault_pk_before;
        svm.expire_blockhash();

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&authority)];
        test_invalid_accounts!(
            svm.clone(),
            authority.pubkey(),
            signers,
            push_ix.clone(),
            {
                // modify mint pubkey (wont match config)
                7 => invalid_program_id(InstructionError::InvalidAccountData, "Mint: Invalid pubkey"),
                // modify token program pubkey
                10 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: Invalid program id"),
                // modify associated token program pubkey
                11 => invalid_program_id(InstructionError::IncorrectProgramId, "AT program: Invalid program id"),
                // modify system program pubkey
                12 => invalid_program_id(InstructionError::IncorrectProgramId, "System program: Invalid program id"),
                // modify instructions sysvars pubkey
                13 => invalid_program_id(InstructionError::IncorrectProgramId, "Instructions sysvar: Invalid instructions sysvar"),
            }
        );

        Ok(())
    }
}
