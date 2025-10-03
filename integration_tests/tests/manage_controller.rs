
mod helpers;
mod subs;

use crate::{
    helpers::{assert::assert_custom_error, setup_test_controller, TestContext},
    subs::{airdrop_lamports, manage_permission},
};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};
use svm_alm_controller::error::SvmAlmControllerErrors;
use svm_alm_controller_client::{
    create_manage_controller_instruction,
    generated::types::{ControllerStatus, PermissionStatus},
};

#[cfg(test)]
mod test {

    use test_case::test_case;

    use super::*;

    #[test_case(false, false, false, false, false, false, false, false, false ; "No permissions")]
    #[test_case(true, false, false, false, false, false, false, false, false ; "Can execute swap")]
    #[test_case(false, true, false, false, false, false, false, false, false ; "Can manage permissions")]
    #[test_case(false, false, true, false, false, false, false, false, false ; "Can invoke external transfer")]
    #[test_case(false, false, false, true, false, false, false, false, false ; "Can reallocate")]
    #[test_case(false, false, false, false, false, true, false, false, false ; "Can unfreeze controller")]
    #[test_case(false, false, false, false, false, false, true, false, false ; "Can manage reserves and integrations")]
    #[test_case(false, false, false, false, false, false, false, true, false ; "Can suspend permissions")]
    #[test_case(false, false, false, false, false, false, false, false, true ; "Can liquidate")]
    fn test_controller_freeze_fails_without_permission(
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

        airdrop_lamports(
            &mut svm,
            &invalid_permission_authority.pubkey(),
            1_000_000_000,
        )?;

        manage_permission(
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

        let instruction = create_manage_controller_instruction(
            &controller_pk,
            &invalid_permission_authority.pubkey(),
            ControllerStatus::Frozen,
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

    #[test_case(false, false, false, false, false, false, false, false, false ; "No permissions")]
    #[test_case(true, false, false, false, false, false, false, false, false ; "Can execute swap")]
    #[test_case(false, true, false, false, false, false, false, false, false ; "Can manage permissions")]
    #[test_case(false, false, true, false, false, false, false, false, false ; "Can invoke external transfer")]
    #[test_case(false, false, false, true, false, false, false, false, false ; "Can reallocate")]
    #[test_case(false, false, false, false, true, false, false, false, false ; "Can freeze controller")]
    #[test_case(false, false, false, false, false, false, true, false, false ; "Can manage reserves and integrations")]
    #[test_case(false, false, false, false, false, false, false, true, false ; "Can suspend permissions")]
    #[test_case(false, false, false, false, false, false, false, false, true ; "Can liquidate")]
    fn test_controller_unfreeze_fails_without_permission(
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

        airdrop_lamports(
            &mut svm,
            &invalid_permission_authority.pubkey(),
            1_000_000_000,
        )?;

        manage_permission(
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

        let instruction = create_manage_controller_instruction(
            &controller_pk,
            &invalid_permission_authority.pubkey(),
            ControllerStatus::Active,
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
}
