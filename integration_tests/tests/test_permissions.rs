mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, derive_permission_pda, fetch_permission_account, initialize_contoller,
    manage_permission,
};
use helpers::lite_svm_with_programs;
use litesvm::LiteSVM;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::{
    ControllerStatus, PermissionStatus,
};

#[cfg(test)]
mod tests {

    use super::*;

    struct TestContext {
        pub svm: LiteSVM,
        pub authority: Keypair,
        pub controller_pk: Pubkey,
    }

    fn setup() -> Result<TestContext, Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        let (controller_pk, _) = initialize_contoller(
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
            true, // can_manage_reserves_and_integrations
            true, // can_suspend_permissions
        )?;

        Ok(TestContext {
            svm,
            authority,
            controller_pk,
        })
    }

    #[test]
    fn test_suspend_permission() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            authority,
        } = setup()?;
        let suspend_authority = Keypair::new();
        airdrop_lamports(&mut svm, &suspend_authority.pubkey(), 1_000_000_000)?;
        let suspended_authority = Keypair::new();
        // Create Permission with can_suspend_permission
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,                  // payer
            &authority,                  // calling authority
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
        )?;
        // Create Permission w/o manage to be suspended
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,                    // payer
            &authority,                    // calling authority
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
}
