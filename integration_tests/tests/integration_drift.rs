mod helpers;
mod subs;

#[cfg(test)]
mod tests {

    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{setup_test_controller, TestContext},
        subs::{fetch_integration_account, initialize_mint, initialize_reserve},
    };
    use solana_sdk::{clock::Clock, pubkey::Pubkey, signer::Signer, transaction::Transaction};
    use svm_alm_controller_client::{
        generated::types::{
            DriftConfig, IntegrationConfig, IntegrationStatus, IntegrationUpdateEvent,
            ReserveStatus, SvmAlmControllerEvent,
        },
        initialize_integration::create_drift_initialize_integration_instruction,
    };
    use test_case::test_case;

    #[test_case(spl_token::ID ; "SPL Token")]
    #[test_case(spl_token_2022::ID ; "Token2022")]
    fn initiailize_drift_success(token_program: Pubkey) -> Result<(), Box<dyn std::error::Error>> {
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

        // Initialize Drift Integration
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
            &mint,
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

        match integration.clone().config {
            IntegrationConfig::Drift(c) => {
                assert_eq!(
                    c,
                    DriftConfig {
                        padding: [0u8; 224]
                    }
                )
            }
            _ => panic!("invalid config"),
        };

        // TODO assert UserStats created
        // TODO assert User created

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
}
