mod helpers;
mod subs;

#[cfg(test)]
mod tests {
    use crate::{
        assert_contains_controller_cpi_event, helpers::{TestContext, setup_test_controller}, subs::{
            derive_controller_authority_pda, fetch_integration_account, initialize_mint, setup_pool_with_token
        }
    };
    use solana_sdk::{clock::Clock, signer::Signer, transaction::Transaction};
    use spl_token::ID as TOKEN_PROGRAM_ID;
    use svm_alm_controller_client::{generated::types::{IntegrationConfig, IntegrationState, IntegrationStatus, IntegrationUpdateEvent, PsmSwapConfig, SvmAlmControllerEvent}, initialize_integration::create_psm_swap_initialize_integration_instruction};
    use borsh::BorshDeserialize;

    #[test]
    fn test_psm_swap_init_success()-> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        // initialize a PSM Pool
        let (
            pool_pda,
            token_pda,
            token_vault,
        ) = setup_pool_with_token(
            &mut svm, 
            &super_authority, 
            &liquidity_mint, 
            false, 
            false,
            &controller_authority
        );
        let rate_limit_slope = 10_000_000_000;
        let rate_limit_max_outflow = 10_000_000_000;
        let integration_status = IntegrationStatus::Active;
        let permit_liquidation = true;
        let description = "psm swap";

        let (init_psm_integration_ix, integration_pda) = create_psm_swap_initialize_integration_instruction(
            &super_authority.pubkey(), 
            &controller_pk, 
            &super_authority.pubkey(), 
            &liquidity_mint, 
            description, 
            integration_status, 
            rate_limit_slope, 
            rate_limit_max_outflow, 
            permit_liquidation, 
            &pool_pda, 
            &token_pda, 
            &token_vault
        );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let integration = fetch_integration_account(&svm, &integration_pda)
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
            IntegrationConfig::PsmSwap(config) => {
                assert_eq!(config, PsmSwapConfig {
                    psm_token: token_pda,
                    psm_pool: pool_pda,
                    mint: liquidity_mint,
                    padding: [0; 128]
                });
            }
            _ => panic!("invalid config"),
        }

        let psm_state = match integration.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        assert_eq!(psm_state.liquidity_supplied, 0);

        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: integration_pda,
            authority: super_authority.pubkey(),
            old_state: None,
            new_state: Some(integration),
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            expected_event
        );
        
        Ok(())
    }

}