mod helpers;
mod subs;
use crate::helpers::constants::USDC_TOKEN_MINT_PUBKEY;
use crate::subs::{
    controller::manage_controller, derive_controller_authority_pda, edit_ata_amount,
    initialize_ata, initialize_reserve, manage_permission, transfer_tokens,
};
use helpers::{
    cctp::evm_address_to_solana_pubkey, constants::CCTP_REMOTE_DOMAIN_ETH, setup_test_controller,
    TestContext,
};
use solana_sdk::signer::Signer;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationStatus, PermissionStatus, ReserveStatus,
};

#[cfg(test)]
mod tests {

    use crate::test_invalid_accounts;
    use crate::{
        helpers::{
            assert::assert_custom_error,
            constants::{
                CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID, CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            },
        },
        subs::{
            airdrop_lamports, fetch_integration_account, fetch_reserve_account,
            get_mint_supply_or_zero, get_token_balance_or_zero,
        },
    };
    use borsh::BorshDeserialize;
    use solana_sdk::account::Account;
    use solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        pubkey::Pubkey,
        signature::Keypair,
        transaction::{Transaction, TransactionError},
    };
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::generated::types::{InitializeArgs, IntegrationType};
    use svm_alm_controller_client::{
        create_cctp_bridge_initialize_integration_instruction, create_cctp_bridge_push_instruction,
        generated::{
            instructions::InitializeIntegrationBuilder,
            types::{
                AccountingAction, AccountingDirection, AccountingEvent, IntegrationConfig,
                IntegrationUpdateEvent, SvmAlmControllerEvent,
            },
        },
    };
    use test_case::test_case;

    use super::*;

    #[test]
    fn cctp_init_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        let clock = svm.get_sysvar::<Clock>();

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;

        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        let integration = fetch_integration_account(&svm, &cctp_usdc_eth_bridge_integration_pk)
            .expect("integration should exist")
            .unwrap();

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
            IntegrationConfig::CctpBridge(c) => {
                assert_eq!(
                    c.cctp_message_transmitter,
                    CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID
                );
                assert_eq!(
                    c.cctp_token_messenger_minter,
                    CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID
                );
                assert_eq!(c.destination_address, destination_address);
                assert_eq!(c.destination_domain, CCTP_REMOTE_DOMAIN_ETH);
                assert_eq!(c.mint, USDC_TOKEN_MINT_PUBKEY);
            }
            _ => panic!("invalid config"),
        };

        // assert emitted event
        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: cctp_usdc_eth_bridge_integration_pk,
            authority: super_authority.pubkey(),
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
    fn cctp_push_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_rate_limit_max_outflow = 1_000_000_000_000;
        let usdc_reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000,              // rate_limit_slope
            reserve_rate_limit_max_outflow, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        let usdc_vault_start_amount = 500_000_000;
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            usdc_vault_start_amount,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let integration_rate_mint_max_outflow = 1_000_000_000_000;
        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000,
            integration_rate_mint_max_outflow,
            false,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let usdc_mint_supply_before = get_mint_supply_or_zero(&svm, &USDC_TOKEN_MINT_PUBKEY);

        let amount = 1_000_000;
        let message_sent_event_data_kp = Keypair::new();
        let push_ix = create_cctp_bridge_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &cctp_usdc_eth_bridge_integration_pk,
            &usdc_reserve_keys.pubkey,
            &message_sent_event_data_kp.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            CCTP_REMOTE_DOMAIN_ETH,
            amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority, &message_sent_event_data_kp],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).unwrap();

        let integration_after =
            fetch_integration_account(&svm, &cctp_usdc_eth_bridge_integration_pk)
                .unwrap()
                .unwrap();
        let usdc_reserve_after = fetch_reserve_account(&svm, &usdc_reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let usdc_balance_after = get_token_balance_or_zero(&svm, &usdc_reserve_keys.vault);
        let usdc_mint_supply_after = get_mint_supply_or_zero(&svm, &USDC_TOKEN_MINT_PUBKEY);

        // Assert Integration rate limits adjusted
        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_rate_mint_max_outflow - amount
        );
        // Assert Reserve rate limits adjusted
        assert_eq!(
            usdc_reserve_after.rate_limit_outflow_amount_available,
            reserve_rate_limit_max_outflow - amount
        );
        // Assert Reserve vault was debited exact amount
        assert_eq!(usdc_balance_after, usdc_vault_start_amount - amount);

        // Assert USDC was burned (i.e. supply decreased)
        let usdc_supply_delta = usdc_mint_supply_before - usdc_mint_supply_after;
        assert_eq!(usdc_supply_delta, amount);

        // Assert accounting events are emitted
        let usdc_vault_delta = usdc_vault_start_amount
            .checked_sub(usdc_balance_after)
            .unwrap();
        let expected_debit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            reserve: Some(usdc_reserve_keys.pubkey),
            mint: USDC_TOKEN_MINT_PUBKEY,
            action: AccountingAction::BridgeSend,
            delta: usdc_vault_delta,
            direction: AccountingDirection::Debit,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            expected_debit_event
        );

        let expected_credit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(cctp_usdc_eth_bridge_integration_pk),
            reserve: None,
            mint: USDC_TOKEN_MINT_PUBKEY,
            action: AccountingAction::BridgeSend,
            delta: usdc_vault_delta,
            direction: AccountingDirection::Credit,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            expected_credit_event
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
    fn cctp_permissions(
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
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let usdc_reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            permit_liquidation,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let push_authority = Keypair::new();
        airdrop_lamports(&mut svm, &push_authority.pubkey(), 1_000_000_000)?;
        // Update the authority to have permissions
        manage_permission(
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

        // Push the integration -- i.e. bridge using CCTP
        let amount = 1_000_000;
        let message_sent_event_data_kp = Keypair::new();
        let push_ix = create_cctp_bridge_push_instruction(
            &controller_pk,
            &push_authority.pubkey(),
            &cctp_usdc_eth_bridge_integration_pk,
            &usdc_reserve_keys.pubkey,
            &message_sent_event_data_kp.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            CCTP_REMOTE_DOMAIN_ETH,
            amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&push_authority.pubkey()),
            &[&push_authority, &message_sent_event_data_kp],
            svm.latest_blockhash(),
        );
        let tx_res = svm.send_transaction(tx);

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
    fn test_cctp_bridge_init_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;

        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            false,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );

        let tx = Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_res = svm.send_transaction(tx);

        assert_custom_error(&tx_res, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test]
    fn test_cctp_bridge_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let usdc_reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            false,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Try to push the integration -- i.e. bridge using CCTP
        let amount = 1_000_000;
        let message_sent_event_data_kp = Keypair::new();
        let push_ix = create_cctp_bridge_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &cctp_usdc_eth_bridge_integration_pk,
            &usdc_reserve_keys.pubkey,
            &message_sent_event_data_kp.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            CCTP_REMOTE_DOMAIN_ETH,
            amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority, &message_sent_event_data_kp],
            svm.latest_blockhash(),
        );
        let tx_res = svm.send_transaction(tx);

        assert_custom_error(
            &tx_res,
            0,
            SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction,
        );

        Ok(())
    }

    #[test]
    fn init_cctp_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;

        let mut init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );

        // Test invalid accounts using the new helper macro
        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&super_authority)];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            init_integration_ix.clone(),
            {
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: Invalid owner"),
                8 => invalid_program_id(InstructionError::InvalidAccountData, "Mint: Invalid mint (does not match local token)"),
                11 => invalid_program_id(InstructionError::IncorrectProgramId, "Invalid cctp_message_transmitter"),
                12 => invalid_program_id(InstructionError::IncorrectProgramId, "Invalid cctp_token_messenger_minter"),
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Local token invalid owner"),
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Remote token messenger invalid owner"),
            }
        )?;

        // invalid destination domain
        svm.expire_blockhash();
        let valid_ix_data = init_integration_ix.data;
        init_integration_ix.data = InitializeIntegrationBuilder::new()
            .integration_type(IntegrationType::CctpBridge)
            .status(IntegrationStatus::Active)
            .description([0u8; 32])
            .rate_limit_slope(100)
            .rate_limit_max_outflow(100)
            .permit_liquidation(true)
            .inner_args(InitializeArgs::CctpBridge {
                destination_address: Pubkey::new_unique(),
                destination_domain: 1234,
            })
            .payer(Pubkey::new_unique())
            .controller(Pubkey::new_unique())
            .controller_authority(Pubkey::new_unique())
            .authority(Pubkey::new_unique())
            .permission(Pubkey::new_unique())
            .integration(Pubkey::new_unique())
            .system_program(Pubkey::new_unique())
            .program_id(Pubkey::new_unique())
            .instruction()
            .data;
        // init_integration_ix.data =
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        init_integration_ix.data = valid_ix_data;

        Ok(())
    }

    #[test]
    fn cctp_push_with_invalid_controller_authority_fails() -> Result<(), Box<dyn std::error::Error>>
    {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_rate_limit_max_outflow = 1_000_000_000_000;
        let usdc_reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000,              // rate_limit_slope
            reserve_rate_limit_max_outflow, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        let usdc_vault_start_amount = 500_000_000;
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            usdc_vault_start_amount,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let integration_rate_mint_max_outflow = 1_000_000_000_000;
        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000,
            integration_rate_mint_max_outflow,
            false,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let amount = 1_000_000;
        let message_sent_event_data_kp = Keypair::new();
        let mut push_ix = create_cctp_bridge_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &cctp_usdc_eth_bridge_integration_pk,
            &usdc_reserve_keys.pubkey,
            &message_sent_event_data_kp.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            CCTP_REMOTE_DOMAIN_ETH,
            amount,
        );

        // Modify controller authority (index 1) to a different pubkey
        push_ix.accounts[1].pubkey = Pubkey::new_unique();

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority, &message_sent_event_data_kp],
            svm.latest_blockhash(),
        );

        let tx_result = svm.send_transaction(tx);

        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidControllerAuthority,
        );

        Ok(())
    }

    #[test]
    fn cctp_push_inner_ctx_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_rate_limit_max_outflow = 1_000_000_000_000;
        let usdc_reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000,              // rate_limit_slope
            reserve_rate_limit_max_outflow, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        let usdc_vault_start_amount = 500_000_000;
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            usdc_vault_start_amount,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let integration_rate_mint_max_outflow = 1_000_000_000_000;
        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000,
            integration_rate_mint_max_outflow,
            false,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let amount = 1_000_000;
        let message_sent_event_data_kp = Keypair::new();
        let push_ix = create_cctp_bridge_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &cctp_usdc_eth_bridge_integration_pk,
            &usdc_reserve_keys.pubkey,
            &message_sent_event_data_kp.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            CCTP_REMOTE_DOMAIN_ETH,
            amount,
        );

        // Checks for inner_ctx accounts:
        // (index 7) mint: owned by SplToken / Token2022, pubkey == config.mint, pubkey == reserve.mint
        // (index 8) vault: pubkey == reserve.vault
        // (index 12) remote_token_messenger: owned by config.cctp_token_messenger_minter, (deserialized).domain == destination_domain
        // (index 14) local_token: owned by config.cctp_token_messenger_minter, (deserialized).mint == inner_ctx.mint
        // (index 16) cctp_message_transmitter: pubkey == config.cctp_message_transmitter
        // (index 17) cctp_token_messenger_minter: pubkey == config.cctp_token_messenger_minter
        // (index 19) token_program: pubkey == SplToken / Token2022
        // (index 20) system_program: pubkey == systen_program_id

        // change remote_token_messenger data
        let remote_token_messenger_pk = push_ix.accounts[12].pubkey;
        let remote_token_messenger_acc_before = svm
            .get_account(&remote_token_messenger_pk)
            .expect("failed to get account");
        let remote_token_messenger_acc_after = Account {
            data: [0; 44].to_vec(), // 8 bytes for discriminator + 4 bytes for domain + 32 bytes for token_message
            ..remote_token_messenger_acc_before
        };
        svm.set_account(remote_token_messenger_pk, remote_token_messenger_acc_after)
            .expect("failed to set account");
        let tx = Transaction::new_signed_with_payer(
            &[push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority, &message_sent_event_data_kp],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        svm.set_account(remote_token_messenger_pk, remote_token_messenger_acc_before)
            .expect("failed to get account");
        svm.expire_blockhash();

        // change local_token data
        let local_token_pk = push_ix.accounts[14].pubkey;
        let local_token_acc_before = svm
            .get_account(&local_token_pk)
            .expect("failed to get account");
        let local_token_acc_after = Account {
            data: [0; 130].to_vec(),
            ..local_token_acc_before
        };
        svm.set_account(local_token_pk, local_token_acc_after)
            .expect("failed to set account");
        let tx = Transaction::new_signed_with_payer(
            &[push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority, &message_sent_event_data_kp],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        svm.set_account(local_token_pk, local_token_acc_before)
            .expect("failed to get account");
        svm.expire_blockhash();

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![
            Box::new(&super_authority),
            Box::new(&message_sent_event_data_kp),
        ];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            push_ix.clone(),
            {
                // Change mint owner:
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: Invalid owner"),
                // Change mint pubkey:
                7 => invalid_program_id(InstructionError::InvalidAccountData, "Mint: invalid pubkey"),
                // Change vault pubkey:
                8 => invalid_program_id(InstructionError::InvalidAccountData, "Vault: invalid pubkey"),
                // Change remote_token_messenger owner
                12 => invalid_owner(InstructionError::InvalidAccountOwner, "Remote Token Messenger: invalid owner"),
                // Change local_token owner
                14 => invalid_owner(InstructionError::InvalidAccountOwner, "Local Token: invalid owner"),
                // Change cctp_message_transmitter pubkey
                16 => invalid_program_id(InstructionError::IncorrectProgramId, "CCTP Message Transmitter: invalid program id"),
                // Change cctp_token_messenger_minter pubkey
                17 => invalid_program_id(InstructionError::IncorrectProgramId, "CCTP Token Messenger Minter: invalid program id"),
                // Change token program id:
                19 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: Invalid id"),
                // Change system program id:
                20 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: Invalid id"),
            }
        );

        Ok(())
    }
}
