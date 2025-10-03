mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, derive_permission_pda,
    fetch_permission_account, initialize_ata, initialize_mint, manage_controller,
    manage_permission, mint_tokens,
};
use helpers::{setup_test_controller, TestContext};
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::{ControllerStatus, PermissionStatus};

#[cfg(test)]
mod tests {
    use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey, transaction::Transaction};
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::create_manage_permissions_instruction;

    use crate::helpers::assert::assert_custom_error;

    use super::*;

    #[test]
    fn test_suspend_permission() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;
        let suspend_authority = Keypair::new();
        airdrop_lamports(&mut svm, &suspend_authority.pubkey(), 1_000_000_000)?;
        let suspended_authority = Keypair::new();
        // Create Permission with can_suspend_permission
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,            // payer
            &super_authority,            // calling authority
            &suspend_authority.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            true,  // can_suspend_permissions
            false, // can_liquidate
        )?;
        // Create Permission w/o manage to be suspended
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,              // payer
            &super_authority,              // calling authority
            &suspended_authority.pubkey(), // subject authority
            PermissionStatus::Active,
            true,  // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;
        // Invoke
        manage_permission(
            &mut svm,
            &controller_pk,
            &suspend_authority,            // payer
            &suspend_authority,            // calling authority
            &suspended_authority.pubkey(), // subject authority
            PermissionStatus::Suspended,
            true,  // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;
        let suspended_permission_pda =
            derive_permission_pda(&controller_pk, &suspended_authority.pubkey());
        let suspended_permission = fetch_permission_account(&mut svm, &suspended_permission_pda)
            .unwrap()
            .unwrap();
        // Validate status is now Suspended
        assert_eq!(suspended_permission.status, PermissionStatus::Suspended);
        Ok(())
    }

    #[test]
    fn test_controller_freeze_unfreeze_permissions() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let freezer = Keypair::new();
        let unfreezer = Keypair::new();
        let regular_user = Keypair::new();

        // Airdrop to all users
        airdrop_lamports(&mut svm, &freezer.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &unfreezer.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &regular_user.pubkey(), 1_000_000_000)?;

        // Initialize a mint
        let usdc_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &usdc_mint,
        )?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &usdc_mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

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

        // Create a permission for unfreezer (can only unfreeze)
        let _unfreezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,    // payer
            &super_authority,    // calling authority
            &unfreezer.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            true,  // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        // Create a permission for regular user (no freeze/unfreeze permissions)
        let _regular_user_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,       // payer
            &super_authority,       // calling authority
            &regular_user.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        // Test 1: Authority can freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Test 2: Authority can unfreeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Active,
        )?;

        // Test 3: Freezer can freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &freezer, // payer
            &freezer, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Test 4: Freezer cannot unfreeze the controller (should fail)
        let freezer_unfreeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &freezer, // payer
            &freezer, // calling authority
            ControllerStatus::Active,
        );
        assert!(
            freezer_unfreeze_result.is_err(),
            "Freezer should not be able to unfreeze controller"
        );

        // Test 5: Unfreezer cannot freeze the controller (should fail)
        let unfreezer_freeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &unfreezer, // payer
            &unfreezer, // calling authority
            ControllerStatus::Frozen,
        );
        assert!(
            unfreezer_freeze_result.is_err(),
            "Unfreezer should not be able to freeze controller"
        );

        // Test 6: Unfreezer can unfreeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &unfreezer, // payer
            &unfreezer, // calling authority
            ControllerStatus::Active,
        )?;

        // Test 7: Regular user cannot freeze the controller (should fail)
        let regular_user_freeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &regular_user, // payer
            &regular_user, // calling authority
            ControllerStatus::Frozen,
        );
        assert!(
            regular_user_freeze_result.is_err(),
            "Regular user should not be able to freeze controller"
        );

        // Test 8: Regular user cannot unfreeze the controller (should fail)
        let regular_user_unfreeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &regular_user, // payer
            &regular_user, // calling authority
            ControllerStatus::Active,
        );
        assert!(
            regular_user_unfreeze_result.is_err(),
            "Regular user should not be able to unfreeze controller"
        );

        Ok(())
    }

    #[test]
    fn test_manage_permission_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let regular_user = Keypair::new();

        // Airdrop to all users
        airdrop_lamports(&mut svm, &regular_user.pubkey(), 1_000_000_000)?;

        // Initialize a mint
        let usdc_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &usdc_mint,
        )?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &usdc_mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Try to manage permission when frozen - should fail

        let instruction = create_manage_permissions_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &super_authority.pubkey(),
            &regular_user.pubkey(),
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
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
    fn test_manage_permission_fails_with_invalid_controller_authority() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let regular_user = Keypair::new();

        // Airdrop to all users
        airdrop_lamports(&mut svm, &regular_user.pubkey(), 1_000_000_000)?;

        // Initialize a mint
        let usdc_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &usdc_mint,
        )?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &usdc_mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        let mut instruction = create_manage_permissions_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &super_authority.pubkey(),
            &regular_user.pubkey(),
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        ); 

        // modify controller authority to an invalid pubkey
        let invalid_controller_authority = Pubkey::new_unique();
        instruction.accounts[2] = AccountMeta {
            pubkey: invalid_controller_authority,
            is_signer: false,
            is_writable: false
        };

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
}
