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
mod tests {

    use solana_sdk::{instruction::InstructionError, pubkey::Pubkey};
    use svm_alm_controller_client::generated::instructions::ManageControllerBuilder;
    use test_case::test_case;

    use crate::{
        helpers::lite_svm_with_programs,
        subs::{derive_controller_authority_pda, initialize_contoller},
    };

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

    #[test]
    fn test_manage_controller_with_invalid_controller_authority_fails(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let payer_and_authority = Keypair::new();
        airdrop_lamports(&mut svm, &payer_and_authority.pubkey(), 10_000_000_000)?;

        let (controller_pk, permission_pda) = initialize_contoller(
            &mut svm,
            &payer_and_authority,
            &payer_and_authority,
            ControllerStatus::Active,
            0,
        )?;

        // Update the authority to have all permissions
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &payer_and_authority,          // payer
            &payer_and_authority,          // calling authority
            &payer_and_authority.pubkey(), // subject authority
            PermissionStatus::Active,
            true, // can_execute_swap,
            true, // can_manage_permissions,
            true, // can_invoke_external_transfer,
            true, // can_reallocate,
            true, // can_freeze,
            true, // can_unfreeze,
            true, // can_manage_reserves_and_integrations
            true, // can_suspend_permissions
            true, // can_liquidate
        )?;

        // invalid controller authority should throw InvalidControllerAuthority error
        let invalid_controller_authority = Pubkey::new_unique();

        let ixn = ManageControllerBuilder::new()
            .status(ControllerStatus::Active)
            .controller(controller_pk)
            .controller_authority(invalid_controller_authority)
            .authority(payer_and_authority.pubkey())
            .permission(permission_pda)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .instruction();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidControllerAuthority,
        );

        Ok(())
    }

    #[test]
    fn test_manage_controller_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let payer_and_authority = Keypair::new();
        airdrop_lamports(&mut svm, &payer_and_authority.pubkey(), 10_000_000_000)?;

        let (controller_pk, permission_pda) = initialize_contoller(
            &mut svm,
            &payer_and_authority,
            &payer_and_authority,
            ControllerStatus::Active,
            0,
        )?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let manage_controller_ix = ManageControllerBuilder::new()
            .status(ControllerStatus::Active)
            .controller(controller_pk)
            .controller_authority(controller_authority)
            .authority(payer_and_authority.pubkey())
            .permission(permission_pda)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .instruction();

        // account checks:
        // (index 0) controller: mut, owner == crate::ID
        // (index 3) permission: owner == crate::ID
        // (index 4) program_id: pubkey == crate::ID

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> =
            vec![Box::new(&payer_and_authority)];
        test_invalid_accounts!(
            svm.clone(),
            payer_and_authority.pubkey(),
            signers,
            manage_controller_ix.clone(),
            {
                // modify controller owner
                0 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller: invalid owner"),
                // modify permission owner
                3 => invalid_owner(InstructionError::InvalidAccountOwner, "Permission: invalid owner"),
                // modify program pubkey
                4 => invalid_program_id(InstructionError::IncorrectProgramId, "Program: invalid ID"),
            }
        );

        Ok(())
    }
}
