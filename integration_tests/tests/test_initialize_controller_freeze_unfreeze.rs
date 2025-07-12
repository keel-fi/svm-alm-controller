mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, derive_controller_authority_pda, initialize_ata, initialize_contoller,
    initialize_integration, initialize_mint, initialize_reserve, manage_controller,
    manage_integration, manage_permission, manage_reserve, mint_tokens, push_integration,
};
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::generated::types::SplTokenExternalConfig;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_controller_freeze_unfreeze_permissions() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();
        let freezer = Keypair::new();
        let unfreezer = Keypair::new();
        let regular_user = Keypair::new();

        // Airdrop to all users
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &freezer.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &unfreezer.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &regular_user.pubkey(), 1_000_000_000)?;

        // Initialize a mint
        let usdc_mint = initialize_mint(&mut svm, &authority, &authority.pubkey(), None, 6, None)?;

        let _authority_usdc_ata =
            initialize_ata(&mut svm, &authority, &authority.pubkey(), &usdc_mint)?;

        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &usdc_mint,
            &authority.pubkey(),
            1_000_000,
        )?;

        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            321u16, // Id
        )?;

        // Update the authority to have all permissions
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &authority.pubkey(), // subject authority
            PermissionStatus::Active,
            true, // can_execute_swap,
            true, // can_manage_permissions,
            true, // can_invoke_external_transfer,
            true, // can_reallocate,
            true, // can_freeze,
            true, // can_unfreeze,
            true, // can_manage_integrations
            true, // can_suspend_permissions
        )?;

        // Create a permission for freezer (can only freeze)
        let _freezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &freezer.pubkey(),   // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_integrations
            false, // can_suspend_permissions
        )?;

        // Create a permission for unfreezer (can only unfreeze)
        let _unfreezer_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &unfreezer.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            true,  // can_unfreeze,
            false, // can_manage_integrations
            false, // can_suspend_permissions
        )?;

        // Create a permission for regular user (no freeze/unfreeze permissions)
        let _regular_user_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &regular_user.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_integrations
            false, // can_suspend_permissions
        )?;

        // Test 1: Authority can freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            ControllerStatus::Suspended,
        )?;

        // Test 2: Authority can unfreeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            ControllerStatus::Active,
        )?;

        // Test 3: Freezer can freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &freezer,            // payer
            &freezer,            // calling authority
            ControllerStatus::Suspended,
        )?;

        // Test 4: Freezer cannot unfreeze the controller (should fail)
        let freezer_unfreeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &freezer,            // payer
            &freezer,            // calling authority
            ControllerStatus::Active,
        );
        assert!(freezer_unfreeze_result.is_err(), "Freezer should not be able to unfreeze controller");

        // Test 5: Unfreezer cannot freeze the controller (should fail)
        let unfreezer_freeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &unfreezer,          // payer
            &unfreezer,          // calling authority
            ControllerStatus::Suspended,
        );
        assert!(unfreezer_freeze_result.is_err(), "Unfreezer should not be able to freeze controller");

        // Test 6: Unfreezer can unfreeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &unfreezer,          // payer
            &unfreezer,          // calling authority
            ControllerStatus::Active,
        )?;

        // Test 7: Regular user cannot freeze the controller (should fail)
        let regular_user_freeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &regular_user,       // payer
            &regular_user,       // calling authority
            ControllerStatus::Suspended,
        );
        assert!(regular_user_freeze_result.is_err(), "Regular user should not be able to freeze controller");

        // Test 8: Regular user cannot unfreeze the controller (should fail)
        let regular_user_unfreeze_result = manage_controller(
            &mut svm,
            &controller_pk,
            &regular_user,       // payer
            &regular_user,       // calling authority
            ControllerStatus::Active,
        );
        assert!(regular_user_unfreeze_result.is_err(), "Regular user should not be able to unfreeze controller");

        Ok(())
    }

} 