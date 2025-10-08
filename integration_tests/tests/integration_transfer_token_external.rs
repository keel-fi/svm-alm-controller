mod helpers;
mod subs;
use crate::subs::{
    controller::manage_controller, derive_controller_authority_pda, initialize_ata, initialize_mint, initialize_reserve,
    manage_permission, manage_reserve, mint_tokens,
};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::generated::types::ReserveStatus;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};

#[cfg(test)]
mod tests {
    use crate::{
        helpers::{assert::assert_custom_error, setup_test_controller, TestContext},
        subs::{
            airdrop_lamports, fetch_integration_account, fetch_reserve_account,
            get_token_balance_or_zero,
        },
    };
    use borsh::BorshDeserialize;
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use super::*;
    use solana_sdk::{
        account::Account, clock::Clock, instruction::InstructionError, transaction::{Transaction, TransactionError}
    };
    use svm_alm_controller_client::{
        create_spl_token_external_initialize_integration_instruction,
        create_spl_token_external_push_instruction, generated::types::{AccountingAction, AccountingDirection, AccountingEvent, IntegrationUpdateEvent, SvmAlmControllerEvent},
    };
    use test_case::test_case;

    #[test_case(spl_token::ID ; "SPL Token")]
    #[test_case(spl_token_2022::ID ; "Token2022")]
    fn spl_token_external_init_success(
        token_program: Pubkey,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            None,
        )?;

        let external = Keypair::new();
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &token_program);

        let _authority_ata =
            initialize_ata(&mut svm, &super_authority, &super_authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        // Initialize a reserve for the token
        let _reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000, // rate_limit_slope
            1_000_000_000, // rate_limit_max_outflow
            &token_program,
        )?;

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &token_program,
            &mint,
            &external.pubkey(),
            &external_ata,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone())
        .map_err(|e| e.err.to_string())?;

        let clock = svm.get_sysvar::<Clock>();

        let integration = fetch_integration_account(&svm, &integration_pubkey)
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
            IntegrationConfig::SplTokenExternal(c) => {
                assert_eq!(c.mint, mint);
                assert_eq!(c.program, token_program);
                assert_eq!(c.recipient, external.pubkey());
                assert_eq!(c.token_account, external_ata);
            }
            _ => panic!("invalid config"),
        };

        // Assert emitted event
        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: integration_pubkey,
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

    #[test_case(spl_token::ID, None ; "SPL Token")]
    #[test_case(spl_token_2022::ID, None ; "Token2022")]
    #[test_case(spl_token_2022::ID, Some(100) ; "Token2022 TransferFee 100 bps")]

    fn transfer_token_external_success(
        token_program: Pubkey,
        token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let external = Keypair::new();

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            token_transfer_fee,
        )?;

        let _authority_ata =
            initialize_ata(&mut svm, &super_authority, &super_authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_rate_limit_max_outflow = 1_000_000_000_000;
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Suspended,
            0, // rate_limit_slope
            0, // rate_limit_max_outflow
            &token_program,
        )?;

        // Update the reserve
        manage_reserve(
            &mut svm,
            &controller_pk,
            &mint,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,              // rate_limit_slope
            reserve_rate_limit_max_outflow, // rate_limit_max_outflow
        )?;

        // Initialize an External integration
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &token_program);
        let integration_rate_mint_max_outflow = 1_000_000_000_000;
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            1_000_000_000_000,
            integration_rate_mint_max_outflow,
            false,
            &token_program,
            &mint,
            &external.pubkey(),
            &external_ata,
        );
        let external_integration_pk = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let vault_start_amount = 10_000_000;
        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Push the integration
        let amount = 1_000_000;
        let push_ix = create_spl_token_external_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &external_integration_pk,
            &reserve_keys.pubkey,
            &token_program,
            &mint,
            &external.pubkey(),
            amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).unwrap();

        let integration_after = fetch_integration_account(&svm, &external_integration_pk)
            .unwrap()
            .unwrap();
        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        let recipient_balance_after = get_token_balance_or_zero(&svm, &external_ata);

        // Assert Integration rate limits adjusted
        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_rate_mint_max_outflow - amount
        );
        // Assert Reserve rate limits adjusted
        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_rate_limit_max_outflow - amount
        );
        // Assert Reserve vault was debited exact amount
        assert_eq!(balance_after, vault_start_amount - amount);
        // Assert recipient's token account received the tokens
        let expected_recipient_amount = match token_transfer_fee {
            Some(fee_bps) => amount - amount * u64::from(fee_bps) / 10_000,
            None => amount,
        };
        assert_eq!(recipient_balance_after, expected_recipient_amount);

        let vault_balance_before = vault_start_amount;
        let vault_balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let check_delta = vault_balance_before
            .checked_sub(vault_balance_after)
            .unwrap();
        // Assert accounting events 
        let expected_debit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent { 
            controller: controller_pk, 
            integration: None, 
            reserve: Some(reserve_keys.pubkey),
            mint: mint, 
            action: AccountingAction::ExternalTransfer, 
            delta: check_delta, 
            direction: AccountingDirection::Debit, 
        }); 
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            expected_debit_event 
        );

        let expected_credit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(external_integration_pk),
            reserve: None,
            mint: mint,
            action: AccountingAction::ExternalTransfer,
            delta: check_delta,
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
    #[test_case(false, true, false, false, false, false, false, false, false, false, true; "can_invoke_external_transfer passes")]
    #[test_case(false, false, true, false, false, false, false, false, false, false, false; "can_execute_swap fails")]
    #[test_case(false, false, false, true, false, false, false, false, false, false, false; "can_reallocate fails")]
    #[test_case(false, false, false, false, true, false, false, false, false, false, false; "can_freeze_controller fails")]
    #[test_case(false, false, false, false, false, true, false, false, false, false, false; "can_unfreeze_controller fails")]
    #[test_case(false, false, false, false, false, false, true, false, false, false, false; "can_manage_reserves_and_integrations fails")]
    #[test_case(false, false, false, false, false, false, false, true, false, false, false; "can_suspend_permissions fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, false, false; "can_liquidate w/o permit_liquidation fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, true, false; "can_liquidate w/ permit_liquidation fails")]
    #[tokio::test]
    async fn spl_token_external_permissions(
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

        let external = Keypair::new();

        // Setup Token Program and Controller state

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Initialize an External integration
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &spl_token::ID);
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            permit_liquidation,
            &spl_token::ID,
            &mint,
            &external.pubkey(),
            &external_ata,
        );
        let external_integration_pk = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &controller_authority,
            10_000_000,
        )?;

        // Setup Permission state and invoke push
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

        let amount = 1_000_000;
        let push_ix = create_spl_token_external_push_instruction(
            &controller_pk,
            &push_authority.pubkey(),
            &external_integration_pk,
            &reserve_keys.pubkey,
            &spl_token::ID,
            &mint,
            &external.pubkey(),
            amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&push_authority.pubkey()),
            &[&push_authority],
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
    fn test_spl_token_external_init_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let external = Keypair::new();

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &spl_token::ID);

        let _authority_ata =
            initialize_ata(&mut svm, &super_authority, &super_authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        // Initialize a reserve for the token
        let _reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000, // rate_limit_slope
            1_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            false,
            &spl_token::ID,
            &mint,
            &external.pubkey(),
            &external_ata,
        );

        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_res = svm.send_transaction(tx);

        assert_custom_error(
            &tx_res,
            0,
            SvmAlmControllerErrors::ControllerFrozen,
        );

        Ok(())
    }

    #[test]
    fn test_spl_token_external_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let external = Keypair::new();

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &controller_authority,
            10_000_000,
        )?;

        // Initialize an External integration
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &spl_token::ID);
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            false,
            &spl_token::ID,
            &mint,
            &external.pubkey(),
            &external_ata,
        );
        let external_integration_pk = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
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

        // Try to push the integration
        let amount = 1_000_000;
        let push_ix = create_spl_token_external_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &external_integration_pk,
            &reserve_keys.pubkey,
            &spl_token::ID,
            &mint,
            &external.pubkey(),
            amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
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
    fn spl_token_external_push_fails_with_invalid_controller_authority() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let external = Keypair::new();
        let token_program = spl_token::ID;
        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            None,
        )?;

        let _authority_ata =
            initialize_ata(&mut svm, &super_authority, &super_authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_rate_limit_max_outflow = 1_000_000_000_000;
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Suspended,
            0, // rate_limit_slope
            0, // rate_limit_max_outflow
            &token_program,
        )?;

        // Update the reserve
        manage_reserve(
            &mut svm,
            &controller_pk,
            &mint,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,              // rate_limit_slope
            reserve_rate_limit_max_outflow, // rate_limit_max_outflow
        )?;

        // Initialize an External integration
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &token_program);
        let integration_rate_mint_max_outflow = 1_000_000_000_000;
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            1_000_000_000_000,
            integration_rate_mint_max_outflow,
            false,
            &token_program,
            &mint,
            &external.pubkey(),
            &external_ata,
        );
        let external_integration_pk = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let vault_start_amount = 10_000_000;
        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Push the integration
        let amount = 1_000_000;
        let mut push_ix = create_spl_token_external_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &external_integration_pk,
            &reserve_keys.pubkey,
            &token_program,
            &mint,
            &external.pubkey(),
            amount,
        );

        // Modify controller authority (index 1) to a different pubkey
        push_ix.accounts[1].pubkey = Pubkey::new_unique();

        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
        
    }

    #[test]
    fn spl_token_external_init_inner_ctx_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;
        let token_program = spl_token::ID;
        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            None,
        )?;

        let external = Keypair::new();
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &token_program);

        let _authority_ata =
            initialize_ata(&mut svm, &super_authority, &super_authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        // Initialize a reserve for the token
        initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000, // rate_limit_slope
            1_000_000_000, // rate_limit_max_outflow
            &token_program,
        )?;

        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &token_program,
            &mint,
            &external.pubkey(),
            &external_ata,
        );

        // Checks for inner_ctx accounts:
        // (index 8) mint
        //      owned by spl token or token2022
        // (index 10) token_account
        //      mut, owned by spl token or token2022 or system program
        // (index 11) token program
        //      pubkey == spl token or token2022
        // (index 12) AT program
        //      pubkey == associated token program id

        // change token_account owner
        // checks the case where the account is initialized but not owned by spl token or token2022
        let token_account_pk = init_ix.accounts[10].pubkey;
        svm.set_account(
            token_account_pk, 
            Account {
            lamports: 10000000,
            data: [1; 165].to_vec(),
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0
        })
        .expect("Failed to set account");
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountOwner)
        );
        svm.set_account(
            token_account_pk, 
            Account {
            lamports: 0,
            data: [].to_vec(),
            owner: Pubkey::default(),
            executable: false,
            rent_epoch: 0
        })
        .expect("Failed to set account");
        svm.expire_blockhash();


        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![
            Box::new(&super_authority), 
        ];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            init_ix.clone(),
            {
                // modify mint owner
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: invalid owner"),
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: invalid owner"),
                // modify token program pubkey
                11 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: incorrect program id"),
                // modify associated token program pubkey
                12 => invalid_program_id(InstructionError::IncorrectProgramId, "AT program: incorrect program id"),
            }
        );


        Ok(())
    }

    #[test]
    fn spl_token_external_push_inner_ctx_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;
        let token_program = spl_token::ID;
        let external = Keypair::new();

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            None,
        )?;

        let _authority_ata =
            initialize_ata(&mut svm, &super_authority, &super_authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let reserve_rate_limit_max_outflow = 1_000_000_000_000;
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Suspended,
            0, // rate_limit_slope
            0, // rate_limit_max_outflow
            &token_program,
        )?;

        // Update the reserve
        manage_reserve(
            &mut svm,
            &controller_pk,
            &mint,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,              // rate_limit_slope
            reserve_rate_limit_max_outflow, // rate_limit_max_outflow
        )?;

        // Initialize an External integration
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &token_program);
        let integration_rate_mint_max_outflow = 1_000_000_000_000;
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "DAO Treasury",
            IntegrationStatus::Active,
            1_000_000_000_000,
            integration_rate_mint_max_outflow,
            false,
            &token_program,
            &mint,
            &external.pubkey(),
            &external_ata,
        );
        let external_integration_pk = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let vault_start_amount = 10_000_000;
        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Push the integration
        let amount = 1_000_000;
        let push_ix = create_spl_token_external_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &external_integration_pk,
            &reserve_keys.pubkey,
            &token_program,
            &mint,
            &external.pubkey(),
            amount,
        );

        // Checks for inner_ctx accounts:
        // (index 8) mint
        //      pubkey == config.mint
        //      owner == config.program
        //      pubkey == reserve.mint
        // (index 9) vault
        //      mut,
        //      pubkey == reserve.vault
        // (index 10) recipient
        //      pubkey == config.recipient
        // (index 11) recipient_token_account
        //      pubkey == config.token_account
        //      owner == ctx.token_program or system program
        // (index 12) token_program 
        //      pubkey == config.program
        // (index 13) AT program
        //      pubkey == AT program id
        // (index 14) system program
        //      pubkey == system program id

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![
            Box::new(&super_authority), 
        ];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            push_ix.clone(),
            {
                // change mint owner
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: invalid owner"),
                // change mint pubkey
                8 => invalid_program_id(InstructionError::InvalidAccountData, "Mint: invalid pubkey"),
                // change vault pubkey
                9 => invalid_program_id(InstructionError::InvalidAccountData, "Vault: invalid pubkey"),
                // change recipient pubkey
                10 => invalid_program_id(InstructionError::InvalidAccountData, "Recipient: invalid pubkey"),
                // change recipient token account pubkey
                11 => invalid_program_id(InstructionError::InvalidAccountData, "Recipient token account: invalid pubkey"),
                // change recipient token account owner
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Recipient token account: invalid owner"),
                // modify token program id
                12 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: Invalid program id"),
                // modify at program id
                13 => invalid_program_id(InstructionError::IncorrectProgramId, "AT program: Invalid program id"),
                // modify system program id
                14 => invalid_program_id(InstructionError::IncorrectProgramId, "System program: Invalid program id"),
            }
        );
        Ok(())
    }
}
