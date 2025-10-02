mod helpers;
mod subs;

#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use solana_sdk::{
        pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction,
    };
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        create_manage_integration_instruction, create_spl_token_external_initialize_integration_instruction, create_sync_integration_instruction, generated::types::{ControllerStatus, IntegrationStatus}
    };

    use crate::{
        helpers::{assert::assert_custom_error, lite_svm_with_programs, setup_test_controller, TestContext},
        subs::{fetch_integration_account, initialize_mint, manage_controller, manage_integration},
    };

    const DEFAULT_RATE_LIMIT_SLOPE: u64 = 1_000_000_000_000;
    const DEFAULT_RATE_LIMIT_MAX_OUTFLOW: u64 = 2_000_000_000_000;

    fn create_test_integration(
        svm: &mut LiteSVM,
        controller: &Pubkey,
        authority: &Keypair,
    ) -> Pubkey {
        let permit_liquidation = true;
        // Initialize a mint
        let mint = initialize_mint(
            svm,
            &authority,
            &authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )
        .unwrap();
        let external = Pubkey::new_unique();
        let description = "DAO Treasury".to_string();
        let external_ata = spl_associated_token_account_client::address::get_associated_token_address_with_program_id(
            &external,
            &mint,
            &spl_token::ID,
        );
        let init_ix = create_spl_token_external_initialize_integration_instruction(
            &authority.pubkey(),
            &controller,
            &authority.pubkey(),
            &description,
            IntegrationStatus::Active,
            DEFAULT_RATE_LIMIT_SLOPE,
            DEFAULT_RATE_LIMIT_MAX_OUTFLOW,
            permit_liquidation,
            &spl_token::ID,
            &mint,
            &external,
            &external_ata,
        );
        // Integration PDA is the 6th account in the init_ix
        let integration_pubkey = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        ))
        .unwrap();
        integration_pubkey
    }

    #[test]
    fn test_manage_integration_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let integration_pubkey =
            create_test_integration(&mut svm, &controller_pk, &super_authority);

        let rate_limit_slope = 5_000_000_000_000;
        let rate_limit_max_outflow = 10_000_000_000_000;
        let description = "DAO Treasury 2";
        manage_integration(
            &mut svm,
            &controller_pk,
            &integration_pubkey,
            &super_authority,
            IntegrationStatus::Suspended,
            rate_limit_slope,
            rate_limit_max_outflow,
            description.to_string(),
        )
        .unwrap();

        let integration = fetch_integration_account(&svm, &integration_pubkey)
            .expect("Failed to fetch integration account")
            .unwrap();

        // Assert all values change
        let mut expected_description = [0u8; 32];
        expected_description[..description.len()].copy_from_slice(description.as_bytes());
        assert_eq!(integration.status, IntegrationStatus::Suspended,);
        assert_eq!(integration.description, expected_description);
        assert_eq!(integration.rate_limit_slope, rate_limit_slope,);
        assert_eq!(integration.rate_limit_max_outflow, rate_limit_max_outflow,);
        assert_eq!(integration.controller, controller_pk);

        Ok(())
    }

    #[test]
    fn test_manage_integration_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let integration_pubkey =
            create_test_integration(&mut svm, &controller_pk, &super_authority);

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        let instruction = create_manage_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
            IntegrationStatus::Suspended,
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
    fn test_sync_integration_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {

        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let integration_pubkey =
            create_test_integration(&mut svm, &controller_pk, &super_authority);

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Try to sync integration when frozen - should fail
        let instruction = create_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
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
