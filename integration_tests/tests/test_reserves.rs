mod helpers;
mod subs;
use crate::helpers::constants::USDC_TOKEN_MINT_PUBKEY;
use crate::subs::{controller::manage_controller, initialize_reserve};
use helpers::{assert::assert_custom_error, setup_test_controller, TestContext};
use solana_sdk::signer::Signer;
use svm_alm_controller::error::SvmAlmControllerErrors;
use svm_alm_controller_client::generated::types::{ControllerStatus, ReserveStatus};

#[cfg(test)]
mod tests {

    use solana_sdk::{instruction::InstructionError, pubkey::Pubkey, signature::Keypair, transaction::Transaction};
    use svm_alm_controller_client::{
        create_initialize_reserve_instruction, create_manage_reserve_instruction,
        create_sync_reserve_instruction, generated::types::PermissionStatus,
    };

    use test_case::test_case;

    use crate::subs::{airdrop_lamports, manage_permission};

    use super::*;

    #[test]
    fn test_initialize_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
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
    fn test_initialize_reserve_fails_with_invalid_controller_authority() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let mut instruction = create_initialize_reserve_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
        );

        // modify controller authority (index 2) to a different pubkey
        instruction.accounts[2].pubkey = Pubkey::new_unique();


        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );

        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_manage_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

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
            &super_authority, // payer
            &super_authority, // calling authority
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
    fn test_manage_reserve_fails_with_invalid_controller_authority() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

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

        let mut instruction = create_manage_reserve_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            ReserveStatus::Suspended,
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

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_sync_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

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
            &super_authority, // payer
            &super_authority, // calling authority
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

    #[test_case(false, false, false, false, false, false, false, false, false ; "No permissions")]
    #[test_case(true, false, false, false, false, false, false, false, false ; "Can execute swap")]
    #[test_case(false, true, false, false, false, false, false, false, false ; "Can manage permissions")]
    #[test_case(false, false, true, false, false, false, false, false, false ; "Can invoke external transfer")]
    #[test_case(false, false, false, true, false, false, false, false, false ; "Can reallocate")]
    #[test_case(false, false, false, false, true, false, false, false, false ; "Can freeze controller")]
    #[test_case(false, false, false, false, false, true, false, false, false ; "Can unfreeze controller")]
    #[test_case(false, false, false, false, false, false, false, true, false ; "Can suspend permissions")]
    #[test_case(false, false, false, false, false, false, false, false, true ; "Can liquidate")]
    fn test_initialize_reserve_fails_without_permission(
        can_execute_swap: bool,
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_reallocate: bool,
        can_freeze_controller: bool,
        can_unfreeze_controller: bool,
        can_manage_reserves_and_integrations: bool,
        can_suspend_permissions: bool,
        can_liquidate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let invalid_permission_authority = Keypair::new();

        airdrop_lamports(&mut svm, &invalid_permission_authority.pubkey(), 1_000_000_000)?;

        // Create Permission with the given permissions
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
            can_manage_reserves_and_integrations,
            can_suspend_permissions,
            can_liquidate,
        )?;

        let instruction = create_initialize_reserve_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &invalid_permission_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&invalid_permission_authority.pubkey()),
            &[&super_authority, &invalid_permission_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::UnauthorizedAction);

        Ok(())
    }

    #[test_case(false, false, false, false, false, false, false, false, false ; "No permissions")]
    #[test_case(true, false, false, false, false, false, false, false, false ; "Can execute swap")]
    #[test_case(false, true, false, false, false, false, false, false, false ; "Can manage permissions")]
    #[test_case(false, false, true, false, false, false, false, false, false ; "Can invoke external transfer")]
    #[test_case(false, false, false, true, false, false, false, false, false ; "Can reallocate")]
    #[test_case(false, false, false, false, true, false, false, false, false ; "Can freeze controller")]
    #[test_case(false, false, false, false, false, true, false, false, false ; "Can unfreeze controller")]
    #[test_case(false, false, false, false, false, false, false, true, false ; "Can suspend permissions")]
    #[test_case(false, false, false, false, false, false, false, false, true ; "Can liquidate")]
    fn test_manage_reserve_fails_without_permission(
        can_execute_swap: bool,
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_reallocate: bool,
        can_freeze_controller: bool,
        can_unfreeze_controller: bool,
        can_manage_reserves_and_integrations: bool,
        can_suspend_permissions: bool,
        can_liquidate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let invalid_permission_authority = Keypair::new();

        airdrop_lamports(&mut svm, &invalid_permission_authority.pubkey(), 1_000_000_000)?;

        // Create Permission with the given permissions
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
            can_manage_reserves_and_integrations,
            can_suspend_permissions,
            can_liquidate,
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
        let _tx_result = svm.send_transaction(txn);

        let manage_reserve_instruction = create_manage_reserve_instruction(
            &controller_pk,
            &invalid_permission_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            ReserveStatus::Suspended,
            1000,
            2000,
        );

        let txn = Transaction::new_signed_with_payer(
            &[manage_reserve_instruction],
            Some(&invalid_permission_authority.pubkey()),
            &[&invalid_permission_authority, &invalid_permission_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::UnauthorizedAction);

        Ok(())
    }

    #[test]
    fn test_sync_reserve_fails_with_invalid_controller_authority() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

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

        let mut instruction = create_sync_reserve_instruction(
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
        );

        // modify controller authority (index ) to a different pubkey
        instruction.accounts[1].pubkey = Pubkey::new_unique();

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_manage_reserve_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

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
        let _tx_result = svm.send_transaction(txn);

        let manage_reserve_ix = create_manage_reserve_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            ReserveStatus::Suspended,
            1000,
            2000,
        );

        // account checks:
        // (index 0) controller: owner == crate::ID,
        // (index 3) permission: owner == crate::ID,
        // (index 4) reserve: mut, owner == crate::ID,
        // (index 5) program_id: pubkey == crate::ID

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![
            Box::new(&super_authority)
        ];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            manage_reserve_ix.clone(),
            {
                // modify controller owner
                0 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller: invalid owner"),
                // modify permission owner
                3 => invalid_owner(InstructionError::InvalidAccountOwner, "Permission: invalid owner"),
                // modify reserve owner
                4 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve: invalid owner"),
                // modify program pubkey
                5 => invalid_program_id(InstructionError::IncorrectProgramId, "Program: invalid ID"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_sync_reserve_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

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

        let instruction = create_sync_reserve_instruction(
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
        );

        // account checks
        // (index 0) controller: owner == crate::ID,
        // (index 2) reserve: mut, owner == crate::ID,
        // (index 3) vault: pubkey check

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![
            Box::new(&super_authority)
        ];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            instruction.clone(),
            {
                // modify controller owner
                0 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller: invalid owner"),
                // modify reserve owner
                2 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve: invalid owner"),
                // modify vault pubkey
                3 => invalid_program_id(InstructionError::InvalidAccountData, "Vault: invalid pubkey"),
            }
        );

        Ok(())
    }
}
