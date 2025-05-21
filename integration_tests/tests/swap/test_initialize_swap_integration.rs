#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
    use svm_alm_controller_client::generated::types::{
        AtomicSwapConfig, ControllerStatus, InitializeArgs, IntegrationConfig, IntegrationStatus,
        PermissionStatus,
    };

    use crate::{
        helpers::spl::setup_token_mint,
        subs::{
            initialize_contoller, initialize_integration, manage_permission,
            oracle::set_oracle_price,
        },
    };

    fn lite_svm_with_programs() -> LiteSVM {
        let mut svm = LiteSVM::new();

        // Add the CONTROLLER program
        let controller_program_bytes =
            include_bytes!("../../../target/deploy/svm_alm_controller.so");
        svm.add_program(
            svm_alm_controller_client::SVM_ALM_CONTROLLER_ID,
            controller_program_bytes,
        );

        svm
    }

    #[test_log::test]
    fn test_happy_path_initialize_swap_integration() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let oracle_pubkey = Pubkey::new_unique();
        set_oracle_price(&mut svm, &oracle_pubkey, 1_000_000_000)?;

        let relayer_authority_kp = Keypair::new();
        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        let coin_token_mint = Pubkey::new_unique();
        let pc_token_mint = Pubkey::new_unique();
        let mint_authority = Keypair::new();
        setup_token_mint(&mut svm, &coin_token_mint, 6, &mint_authority.pubkey());
        setup_token_mint(&mut svm, &pc_token_mint, 6, &mint_authority.pubkey());

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp,          // payer
            &relayer_authority_kp,          // calling authority
            &relayer_authority_kp.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            true,  // can_manage_integrations
        )?;

        // Initialize an AtomicSwap integration
        let _atomic_swap_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::AtomicSwap(AtomicSwapConfig {
                input_token: pc_token_mint,
                output_token: coin_token_mint,
                oracle: oracle_pubkey,
                max_slippage_bps: 123,
                padding: [0u8; 94],
            }),
            &InitializeArgs::AtomicSwap {
                max_slippage_bps: 123,
            },
        )?;

        Ok(())
    }
}
