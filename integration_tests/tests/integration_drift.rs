mod helpers;
mod subs;

#[cfg(test)]
mod tests {

    use crate::helpers::drift::state::{
        spot_market::{setup_drift_spot_market_vault, setup_mock_insurance_fund_account},
    };
    use crate::helpers::pyth::oracle::{setup_mock_oracle_account};
    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{
            drift::{set_drift_spot_market, setup_drift_state, User, UserStats},
            setup_test_controller, TestContext,
        },
        subs::{
            fetch_integration_account, initialize_ata, initialize_mint, initialize_reserve,
            mint_tokens,
        },
    };
    use borsh::BorshDeserialize;
    use bytemuck;
    use solana_sdk::signer::keypair::Keypair;
    use solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use spl_token;
    use svm_alm_controller_client::{
        derive_controller_authority_pda,
        generated::types::{
            AccountingAction, AccountingDirection, AccountingEvent, DriftConfig, IntegrationConfig,
            IntegrationStatus, IntegrationUpdateEvent, ReserveStatus, SvmAlmControllerEvent,
        },
        initialize_integration::create_drift_initialize_integration_instruction,
        instructions::create_drift_push_instruction,
        integrations::drift::{derive_user_pda, derive_user_stats_pda},
    };

    #[test]
    fn initiailize_drift_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;
        let spot_market_index = 0;
        setup_drift_state(&mut svm);
        set_drift_spot_market(&mut svm, spot_market_index, None);
        set_drift_spot_market(&mut svm, spot_market_index + 1, None);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone())
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

        match &integration.config {
            IntegrationConfig::Drift(c) => {
                assert_eq!(
                    c,
                    &DriftConfig {
                        sub_account_id,
                        spot_market_index,
                        padding: [0u8; 220]
                    }
                )
            }
            _ => panic!("invalid config"),
        };

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        // Assert UserStats created and authority is controller_authority
        let drift_user_stats_pda = derive_user_stats_pda(&controller_authority);
        let drift_user_stats_acct = svm.get_account(&drift_user_stats_pda).unwrap();
        let drift_user_stats = UserStats::try_from(&drift_user_stats_acct.data).unwrap();
        assert_eq!(drift_user_stats.authority, controller_authority);

        // Assert User created
        let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
        let drift_user_acct = svm.get_account(&drift_user_pda).unwrap();
        let drift_user = User::try_from(&drift_user_acct.data).unwrap();
        assert_eq!(drift_user.authority, controller_authority);
        assert_eq!(drift_user.sub_account_id, sub_account_id);
        assert_eq!(drift_user.total_deposits, 0);
        assert_eq!(drift_user.total_withdraws, 0);

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

        // Creation of a second Integraiton should work without error
        // due to checks UserStats and User exist.
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            // Increment spot market index so integration key is different
            spot_market_index + 1,
        );
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);
        assert!(tx_result.is_ok());

        Ok(())
    }

    #[test]
    fn initiailize_drift_invalid_spot_market_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;
        setup_drift_state(&mut svm);

        let spot_market_index = 0;
        let spot_market_pubkey = set_drift_spot_market(&mut svm, 0, None);

        // overwrite with incorrect market ID
        let mut market = svm.get_account(&spot_market_pubkey).unwrap();
        market.data[684..686].copy_from_slice(&9u16.to_le_bytes());
        svm.set_account(spot_market_pubkey, market).unwrap();

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
        );
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone());
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );

        Ok(())
    }

    #[test]
    fn drift_push_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &spl_token::ID,
            None,
        )?;

        let spot_market_pubkey =
            set_drift_spot_market(&mut svm, spot_market_index, Some(token_mint));

        // Set up mock oracle and insurance fund accounts
        let spot_market_account = svm.get_account(&spot_market_pubkey).unwrap();
        let spot_market_data = &spot_market_account.data[8..]; // Skip discriminator
        let spot_market = bytemuck::try_from_bytes::<
            crate::helpers::drift::state::spot_market::SpotMarket,
        >(spot_market_data)
        .unwrap();

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &spl_token::ID);

        // Set up mock oracle and insurance fund accounts
        let spot_market_account = svm.get_account(&spot_market_pubkey).unwrap();
        let spot_market_data = &spot_market_account.data[8..]; // Skip discriminator
        let spot_market = bytemuck::try_from_bytes::<
            crate::helpers::drift::state::spot_market::SpotMarket,
        >(spot_market_data)
        .unwrap();

        setup_mock_oracle_account(&mut svm, &spot_market.oracle);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Create associated token account for controller authority and mint tokens
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let vault_start_amount = 1_000_000_000;

        // Initialize ATA for controller authority
        let ata_pubkey = initialize_ata(
            &mut svm,
            &super_authority,
            &controller_authority,
            &token_mint,
        )?;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Create the push instruction
        let push_amount = 100_000_000;

        // Use the spot market account we already have
        let spot_market_account = svm.get_account(&spot_market_pubkey).unwrap();

        let integration_before = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        let reserve_before = crate::subs::fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .expect("reserve should exist")
            .unwrap();

        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &ata_pubkey, // controller authority's ATA
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            push_amount,
            false,
            &spot_market_account.data, // Pass the spot market account data
        )
        .unwrap();

        // Execute the push instruction
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).unwrap();

        let integration_after = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        let reserve_after = crate::subs::fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .expect("reserve should exist")
            .unwrap();

        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_before.rate_limit_outflow_amount_available - push_amount
        );

        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_before.rate_limit_outflow_amount_available - push_amount
        );

        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: None,
                mint: spot_market.vault,
                reserve: Some(reserve_keys.pubkey),
                direction: AccountingDirection::Debit,
                action: AccountingAction::Deposit,
                delta: push_amount,
            })
        );

        Ok(())
    }
}
