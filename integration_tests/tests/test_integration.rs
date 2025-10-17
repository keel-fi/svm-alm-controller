mod helpers;
mod subs;

#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use solana_sdk::{
        account::Account,
        instruction::InstructionError,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        create_manage_integration_instruction,
        create_spl_token_external_initialize_integration_instruction,
        create_spl_token_external_push_instruction, create_sync_integration_instruction,
        generated::types::{ControllerStatus, IntegrationStatus, PermissionStatus, ReserveStatus},
        SVM_ALM_CONTROLLER_ID,
    };

    use test_case::test_case;

    use crate::{
        helpers::{assert::assert_custom_error, setup_test_controller, TestContext},
        subs::{
            airdrop_lamports, fetch_integration_account, initialize_mint, initialize_reserve,
            manage_controller, manage_integration, manage_permission,
        },
        test_invalid_accounts,
    };

    const DEFAULT_RATE_LIMIT_SLOPE: u64 = 1_000_000_000_000;
    const DEFAULT_RATE_LIMIT_MAX_OUTFLOW: u64 = 2_000_000_000_000;

    fn create_test_integration(
        svm: &mut LiteSVM,
        controller: &Pubkey,
        authority: &Keypair,
    ) -> (Pubkey, Pubkey, Pubkey) {
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
        (integration_pubkey, mint, external)
    }

    #[test]
    fn test_init_integration_fails_with_invalid_controller_authority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let permit_liquidation = true;
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
        )
        .unwrap();
        let external = Pubkey::new_unique();
        let description = "DAO Treasury".to_string();
        let external_ata = spl_associated_token_account_client::address::get_associated_token_address_with_program_id(
            &external,
            &mint,
            &spl_token::ID,
        );
        let mut init_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
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

        // modify controller_authority to a different pubkey
        init_ix.accounts[2].pubkey = Pubkey::new_unique();

        let txn = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidControllerAuthority,
        );

        Ok(())
    }

    #[test]
    fn test_manage_integration_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let (integration_pubkey, _, _) =
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

        let (integration_pubkey, _, _) =
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
    fn test_manage_integration_fails_with_invalid_controller_authority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let (integration_pubkey, _, _) =
            create_test_integration(&mut svm, &controller_pk, &super_authority);

        let mut instruction = create_manage_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
            IntegrationStatus::Suspended,
            1000,
            2000,
        );

        // modify controller authority (index 1) to a different pubkey
        instruction.accounts[1].pubkey = Pubkey::new_unique();

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidControllerAuthority,
        );

        Ok(())
    }

    #[test]
    fn test_sync_integration_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let (integration_pubkey, mint, _) =
            create_test_integration(&mut svm, &controller_pk, &super_authority);

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
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
            &reserve_keys.pubkey,
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

    #[test_case(false, false, false, false, false, false, false, false ; "No permissions")]
    #[test_case(true, false, false, false, false, false, false, false ; "Can execute swap")]
    #[test_case(false, true, false, false, false, false, false, false ; "Can manage permissions")]
    #[test_case(false, false, true, false, false, false, false, false ; "Can invoke external transfer")]
    #[test_case(false, false, false, true, false, false, false, false ; "Can reallocate")]
    #[test_case(false, false, false, false, true, false, false, false ; "Can freeze controller")]
    #[test_case(false, false, false, false, false, true, false, false ; "Can unfreeze controller")]
    #[test_case(false, false, false, false, false, false, true, false ; "Can suspend permissions")]
    #[test_case(false, false, false, false, false, false, false, true ; "Can liquidate")]
    fn test_initialize_integration_fails_without_permission(
        can_execute_swap: bool,
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_reallocate: bool,
        can_freeze_controller: bool,
        can_unfreeze_controller: bool,
        can_suspend_permissions: bool,
        can_liquidate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller().unwrap();

        let invalid_permission_authority = Keypair::new();

        airdrop_lamports(
            &mut svm,
            &invalid_permission_authority.pubkey(),
            1_000_000_000,
        )?;

        // Create Permission with the given permissions (excluding can_manage_reserves_and_integrations)
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,
            &super_authority,
            &invalid_permission_authority.pubkey(),
            PermissionStatus::Active,
            can_execute_swap,
            can_manage_permissions,
            can_invoke_external_transfer,
            can_reallocate,
            can_freeze_controller,
            can_unfreeze_controller,
            false, // can_manage_reserves_and_integrations - this should be false for the test to pass
            can_suspend_permissions,
            can_liquidate,
        )?;

        let instruction = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &invalid_permission_authority.pubkey(),
            &"DAO Treasury".to_string(),
            IntegrationStatus::Active,
            DEFAULT_RATE_LIMIT_SLOPE,
            DEFAULT_RATE_LIMIT_MAX_OUTFLOW,
            true, // permit_liquidation
            &spl_token::ID,
            &initialize_mint(
                &mut svm,
                &super_authority,
                &super_authority.pubkey(),
                None,
                6,
                None,
                &spl_token::ID,
                None,
            ).unwrap(),
            &Pubkey::new_unique(),
            &spl_associated_token_account_client::address::get_associated_token_address_with_program_id(
                &Pubkey::new_unique(),
                &initialize_mint(
                    &mut svm,
                    &super_authority,
                    &super_authority.pubkey(),
                    None,
                    6,
                    None,
                    &spl_token::ID,
                    None,
                ).unwrap(),
                &spl_token::ID,
            ),
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &invalid_permission_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::UnauthorizedAction);

        Ok(())
    }

    #[test_case(false, false, false, false, false, false, false, false ; "No permissions")]
    #[test_case(true, false, false, false, false, false, false, false ; "Can execute swap")]
    #[test_case(false, true, false, false, false, false, false, false ; "Can manage permissions")]
    #[test_case(false, false, true, false, false, false, false, false ; "Can invoke external transfer")]
    #[test_case(false, false, false, true, false, false, false, false ; "Can reallocate")]
    #[test_case(false, false, false, false, true, false, false, false ; "Can freeze controller")]
    #[test_case(false, false, false, false, false, true, false, false ; "Can unfreeze controller")]
    #[test_case(false, false, false, false, false, false, true, false ; "Can suspend permissions")]
    #[test_case(false, false, false, false, false, false, false, true ; "Can liquidate")]
    fn test_manage_integration_fails_without_permission(
        can_execute_swap: bool,
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_reallocate: bool,
        can_freeze_controller: bool,
        can_unfreeze_controller: bool,
        can_suspend_permissions: bool,
        can_liquidate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller().unwrap();

        let invalid_permission_authority = Keypair::new();

        airdrop_lamports(
            &mut svm,
            &invalid_permission_authority.pubkey(),
            1_000_000_000,
        )?;

        // Create Permission with the given permissions (excluding can_manage_reserves_and_integrations)
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,
            &super_authority,
            &invalid_permission_authority.pubkey(),
            PermissionStatus::Active,
            can_execute_swap,
            can_manage_permissions,
            can_invoke_external_transfer,
            can_reallocate,
            can_freeze_controller,
            can_unfreeze_controller,
            false, // can_manage_reserves_and_integrations - this should be false for the test to pass
            can_suspend_permissions,
            can_liquidate,
        )?;

        // Initialize integration first (using super_authority with proper permissions)
        let (integration_pubkey, _, _) =
            create_test_integration(&mut svm, &controller_pk, &super_authority);

        let instruction = create_manage_integration_instruction(
            &controller_pk,
            &invalid_permission_authority.pubkey(),
            &integration_pubkey,
            IntegrationStatus::Suspended,
            1000,
            2000,
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&invalid_permission_authority.pubkey()),
            &[&invalid_permission_authority, &invalid_permission_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::UnauthorizedAction);

        Ok(())
    }

    #[test]
    fn test_init_integration_outer_ctx_invalid_accounts_fails(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller().unwrap();

        // Initialize a mint
        let mint_pk = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )
        .unwrap();

        let external = Pubkey::new_unique();
        let external_ata = spl_associated_token_account_client::address::get_associated_token_address_with_program_id(
            &external,
            &mint_pk,
            &spl_token::ID,
        );

        // In order to test the outer ctx account checks, we can use any integration
        // In this case we use SPL Token External Integration
        let init_integration_ix = create_spl_token_external_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "OuterCtxAccsChecks",
            IntegrationStatus::Active,
            10_000,
            10_000,
            true,
            &spl_token::ID,
            &mint_pk,
            &external,
            &external_ata,
        );

        // Checks for outer ctx accounts:
        // controller - change owner
        // permission - change owner
        // integration - initialize before ix, change owner
        // program_id - change pubkey
        // system program - change pubkey

        // create Integration before creation in ix
        let integration_pk = init_integration_ix.accounts[5].pubkey;
        let account = Account {
            lamports: 1_000_000_000, // arbitrary default lamports
            data: vec![1, 1, 1, 1],
            owner: SVM_ALM_CONTROLLER_ID,
            executable: false,
            rent_epoch: 0,
        };

        svm.set_account(integration_pk, account)?;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::AccountAlreadyInitialized)
        );
        svm.set_account(
            integration_pk,
            Account {
                lamports: 0,
                data: vec![],
                owner: Pubkey::default(),
                executable: false,
                rent_epoch: 0,
            },
        )?;

        svm.expire_blockhash();

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&super_authority)];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            init_integration_ix.clone(),
            {
                // Change controller (index 1) owner:
                1 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller: Invalid owner"),
                // Change permission (index 4) owner:
                4 => invalid_owner(InstructionError::InvalidAccountOwner, "Permission: Invalid owner"),
                // Change integration (index 5) owner:
                5 => invalid_owner(InstructionError::InvalidAccountOwner, "Integration: Invalid owner"),
                // Change program_id (index 6)
                6 => invalid_program_id(InstructionError::IncorrectProgramId, "Program: invalid program id"),
                // Change system_program_id (index 7)
                7 => invalid_program_id(InstructionError::IncorrectProgramId, "Program: invalid program id"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_push_integration_outer_ctx_invalid_accounts_fails(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let (integration_pubkey, mint, recipient) =
            create_test_integration(&mut svm, &controller_pk, &super_authority);

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
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

        let push_ix = create_spl_token_external_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
            &reserve_keys.pubkey,
            &spl_token::ID,
            &mint,
            &recipient,
            1000,
        );

        // Checks for outer ctx accounts:
        // controller - change owner
        // permission - change owner
        // integration - change owner
        // reserve - change owner
        // program_id - change pubkey

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&super_authority)];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            push_ix.clone(),
            {
                // Change controller (index 1) owner:
                0 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller: Invalid owner"),
                // Change permission (index 3) owner:
                3 => invalid_owner(InstructionError::InvalidAccountOwner, "Permission: Invalid owner"),
                // Change integration (index 4) owner:
                4 => invalid_owner(InstructionError::InvalidAccountOwner, "Integration: Invalid owner"),
                // Change reserve (index 5) owner:
                5 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve: Invalid owner"),
                // Change program_id (index 6) owner:
                6 => invalid_program_id(InstructionError::IncorrectProgramId, "Program: invalid program id"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_manage_integration_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            super_authority,
            controller_pk,
        } = setup_test_controller().unwrap();

        let (integration_pubkey, _, _) =
            create_test_integration(&mut svm, &controller_pk, &super_authority);
        let description = String::from("test value");
        let description_bytes = description.as_bytes();
        let mut description_encoding: [u8; 32] = [0; 32];
        description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

        let ix = create_manage_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
            IntegrationStatus::Active,
            1000,
            1000,
        );

        // account checks
        // (index 0) controller: owner == crate::ID,
        // (index 3) permission: owner == crate::ID,
        // (index 4) integration: mut, owner == crate::ID,
        // (index 5) program: pubkey == crate::ID

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&super_authority)];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            ix.clone(),
            {
                // Change controller owner
                0 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller: Invalid owner"),
                // Change permission owner
                3 => invalid_owner(InstructionError::InvalidAccountOwner, "Permission: Invalid owner"),
                // Change integration owner
                4 => invalid_owner(InstructionError::InvalidAccountOwner, "Integration: Invalid owner"),
                // Change program_id program id
                5=> invalid_program_id(InstructionError::IncorrectProgramId, "Program: invalid program id"),
            }
        );

        Ok(())
    }
}
