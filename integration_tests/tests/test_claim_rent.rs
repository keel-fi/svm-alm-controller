mod helpers;
mod subs;

#[cfg(test)]
mod tests {
    use solana_sdk::{
        instruction::InstructionError,
        signature::Keypair,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        claim_rent::create_claim_rent_instruction,
        create_manage_controller_instruction, derive_controller_authority_pda,
        generated::types::{ControllerStatus, PermissionStatus},
    };

    use crate::{
        helpers::{assert::assert_custom_error, setup_test_controller, TestContext},
        subs::{airdrop_lamports, manage_permission},
        test_invalid_accounts,
    };
    use test_case::test_case;

    #[test]
    fn test_claim_rent_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let controller_authority_balance_before = 1_000_000_000;
        airdrop_lamports(
            &mut svm,
            &controller_authority,
            controller_authority_balance_before,
        )?;

        let rent_destination = Keypair::new().pubkey();

        let claim_rent_ix = create_claim_rent_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &rent_destination,
        );
        let txn = Transaction::new_signed_with_payer(
            &[claim_rent_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(txn).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;
        let controller_authority_balance_after = svm
            .get_balance(&controller_authority)
            .expect("Failed to get controller_authority balance");
        let rent_destination_balance_after = svm
            .get_balance(&rent_destination)
            .expect("Failed to get rent_destination balance");

        // assert controller_authority was debited and rent_destination credited
        assert_eq!(controller_authority_balance_after, 0);
        assert_eq!(
            controller_authority_balance_before,
            rent_destination_balance_after
        );

        Ok(())
    }

    #[test]
    fn test_claim_rent_reverts_no_balance() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let rent_destination = Keypair::new().pubkey();

        let claim_rent_ix = create_claim_rent_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &rent_destination,
        );
        let txn = Transaction::new_signed_with_payer(
            &[claim_rent_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_eq!(
            tx_result.clone().err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InsufficientFunds)
        );

        Ok(())
    }

    #[test_case(false, true, true ; "Controller not active, Permission active, can reallocate")]
    #[test_case(true, true, false ; "Controller active, Permission active, can't reallocate")]
    #[test_case(true, false, true ; "Controller active, Permission suspended, can reallocate")]
    fn test_claim_rent_fails_without_valid_controller_status_and_permission(
        is_controller_active: bool,
        is_permission_active: bool,
        can_reallocate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        airdrop_lamports(&mut svm, &controller_authority, 1_000_000_000)?;

        let new_permission_authority = Keypair::new();
        airdrop_lamports(
            &mut svm,
            &new_permission_authority.pubkey(),
            1_000_000_000,
        )?;

        let permission_status = if is_permission_active {
            PermissionStatus::Active
        } else {
            PermissionStatus::Suspended
        };

        manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,
            &super_authority,
            &new_permission_authority.pubkey(),
            permission_status,
            true,
            true,
            true,
            can_reallocate,
            true,
            true,
            true,
            true,
            true,
        )?;

        if !is_controller_active {
            let instruction = create_manage_controller_instruction(
                &controller_pk,
                &super_authority.pubkey(),
                ControllerStatus::Frozen,
            );

            let txn = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&super_authority.pubkey()),
                &[&super_authority],
                svm.latest_blockhash(),
            );
            svm.send_transaction(txn).unwrap();
        }

        let claim_rent_ix = create_claim_rent_instruction(
            &controller_pk,
            &new_permission_authority.pubkey(),
            &Keypair::new().pubkey(),
        );

        let txn = Transaction::new_signed_with_payer(
            &[claim_rent_ix],
            Some(&new_permission_authority.pubkey()),
            &[&new_permission_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        if !is_controller_active {
            assert_custom_error(
                &tx_result,
                0,
                SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction,
            );
        }

        if !can_reallocate {
            assert_eq!(
                tx_result.clone().err().unwrap().err,
                TransactionError::InstructionError(0, InstructionError::IncorrectAuthority)
            )
        }

        if !is_permission_active {
            assert_custom_error(
                &tx_result,
                0,
                SvmAlmControllerErrors::PermissionStatusDoesNotPermitAction,
            );
        }

        Ok(())
    }

    #[test]
    fn test_claim_rent_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        airdrop_lamports(&mut svm, &controller_authority, 1_000_000_000)?;

        let rent_destination = Keypair::new().pubkey();

        let claim_rent_ix = create_claim_rent_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &rent_destination,
        );

        // Accounts are: controller(0), controller_authority(1), authority(2),
        // permission(3), destination(4), system_program(5),

        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            claim_rent_ix.clone(),
            {
                // modify controller owner
                0 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller: invalid owner"),
                // modify controller_authority owner
                1 => invalid_owner(InstructionError::InvalidAccountOwner, "Controller Authority: invalid owner"),
                // modify permission owner
                3 => invalid_owner(InstructionError::InvalidAccountOwner, "Permission: invalid owner"),
                // modify system_program pubkey
                5 => invalid_program_id(InstructionError::IncorrectProgramId, "System Program: invalid ID"),
            }
        );

        Ok(())
    }
}
