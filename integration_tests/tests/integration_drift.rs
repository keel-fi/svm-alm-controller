mod helpers;
mod subs;

#[cfg(test)]
mod tests {

    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{
            drift::{set_drift_spot_market, setup_drift_state, User, UserStats},
            setup_test_controller, TestContext,
        },
        subs::fetch_integration_account,
    };
    use borsh::BorshDeserialize;
    use solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use svm_alm_controller_client::{
        derive_controller_authority_pda,
        generated::types::{
            DriftConfig, IntegrationConfig, IntegrationStatus, IntegrationUpdateEvent,
            SvmAlmControllerEvent,
        },
        initialize_integration::create_drift_initialize_integration_instruction,
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
        set_drift_spot_market(&mut svm, spot_market_index);
        set_drift_spot_market(&mut svm, spot_market_index + 1);

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
        let spot_market_pubkey = set_drift_spot_market(&mut svm, 0);

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
}
