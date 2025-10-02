mod helpers;
mod subs;
use crate::helpers::constants::USDC_TOKEN_MINT_PUBKEY;
use crate::subs::{
    controller::manage_controller, derive_controller_authority_pda, edit_ata_amount,
    initialize_ata, initialize_reserve, manage_permission, transfer_tokens,
};
use helpers::{
    assert::assert_custom_error, cctp::evm_address_to_solana_pubkey,
    constants::CCTP_REMOTE_DOMAIN_ETH, setup_test_controller, TestContext,
};
use solana_sdk::signer::Signer;
use svm_alm_controller::error::SvmAlmControllerErrors;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationStatus, PermissionStatus, ReserveStatus,
};

#[cfg(test)]
mod tests {

    use solana_sdk::{
        clock::Clock, instruction::InstructionError, signature::Keypair, transaction::{Transaction, TransactionError}
    };
    use svm_alm_controller_client::{
        create_cctp_bridge_initialize_integration_instruction, create_cctp_bridge_push_instruction,
        create_initialize_reserve_instruction, create_manage_reserve_instruction,
        create_sync_reserve_instruction, generated::types::IntegrationConfig,
    };
    use test_case::test_case;

    use crate::{
        helpers::constants::{
            CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID, CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
        },
        subs::{
            airdrop_lamports, fetch_integration_account, fetch_reserve_account,
            get_mint_supply_or_zero, get_token_balance_or_zero,
        },
    };

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
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let integration = fetch_integration_account(&svm, &cctp_usdc_eth_bridge_integration_pk)
            .expect("integration should exist")
            .unwrap();

        assert_eq!(integration.controller, controller_pk);
        assert_eq!(integration.status, IntegrationStatus::Active);
        assert_eq!(integration.rate_limit_slope, rate_limit_slope);
        assert_eq!(integration.rate_limit_max_outflow, rate_limit_max_outflow);
        assert_eq!(integration.rate_limit_outflow_amount_available, rate_limit_max_outflow);
        assert_eq!(integration.rate_limit_remainder, 0);
        assert_eq!(integration.permit_liquidation, permit_liquidation);
        assert_eq!(integration.last_refresh_timestamp, clock.unix_timestamp);
        assert_eq!(integration.last_refresh_slot, clock.slot);

        match integration.config {
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
        svm.send_transaction(tx).unwrap();

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

        let freezer = Keypair::new();

        // Airdrop to all users
        airdrop_lamports(&mut svm, &freezer.pubkey(), 1_000_000_000)?;

        // Create a permission for freezer (can only freeze)
        let _freezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,  // payer
            &super_authority,  // calling authority
            &freezer.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &freezer, // payer
            &freezer, // calling authority
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

        let freezer = Keypair::new();

        // Airdrop to all users
        airdrop_lamports(&mut svm, &freezer.pubkey(), 1_000_000_000)?;

        // Create a permission for freezer (can only freeze)
        let _freezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,  // payer
            &super_authority,  // calling authority
            &freezer.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &freezer, // payer
            &freezer, // calling authority
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
    fn test_initialize_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let freezer = Keypair::new();

        // Airdrop to freezer
        airdrop_lamports(&mut svm, &freezer.pubkey(), 1_000_000_000)?;

        // Create a permission for freezer (can only freeze)
        let _freezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,  // payer
            &super_authority,  // calling authority
            &freezer.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &freezer, // payer
            &freezer, // calling authority
            ControllerStatus::Frozen,
        )?;

        let instruction = create_initialize_reserve_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );

        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test]
    fn test_manage_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let freezer = Keypair::new();

        // Airdrop to freezer
        airdrop_lamports(&mut svm, &freezer.pubkey(), 1_000_000_000)?;

        // Create a permission for freezer (can only freeze)
        let _freezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,  // payer
            &super_authority,  // calling authority
            &freezer.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        // Initialize reserve first (while controller is active)
        let _reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            0,
            0,
            &spl_token::ID,
        )?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &freezer, // payer
            &freezer, // calling authority
            ControllerStatus::Frozen,
        )?;

        let instruction = create_manage_reserve_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            ReserveStatus::Suspended,
            1000,
            2000,
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test]
    fn test_sync_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let freezer = Keypair::new();

        // Airdrop to freezer
        airdrop_lamports(&mut svm, &freezer.pubkey(), 1_000_000_000)?;

        // Create a permission for freezer (can only freeze)
        let _freezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,  // payer
            &super_authority,  // calling authority
            &freezer.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        // Initialize reserve first (while controller is active)
        let _reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            0,
            0,
            &spl_token::ID,
        )?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &freezer, // payer
            &freezer, // calling authority
            ControllerStatus::Frozen,
        )?;

        let instruction = create_sync_reserve_instruction(
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
        );
        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }
}
