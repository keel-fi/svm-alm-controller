use borsh::BorshDeserialize;
use litesvm::LiteSVM;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, transaction::Transaction,
};
use std::error::Error;
use svm_alm_controller_client::generated::{
    accounts::Permission,
    instructions::ManagePermissionBuilder,
    programs::SVM_ALM_CONTROLLER_ID,
    types::{PermissionStatus, PermissionUpdateEvent, SvmAlmControllerEvent},
};

use crate::{assert_contains_controller_cpi_event, subs::derive_controller_authority_pda};

pub fn derive_permission_pda(controller_pda: &Pubkey, authority: &Pubkey) -> Pubkey {
    let (permission_pda, _permission_bump) = Pubkey::find_program_address(
        &[
            b"permission",
            &controller_pda.to_bytes(),
            &authority.to_bytes(),
        ],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    permission_pda
}

pub fn fetch_permission_account(
    svm: &mut LiteSVM,
    permission_pda: &Pubkey,
) -> Result<Option<Permission>, Box<dyn Error>> {
    let permission_info = svm.get_account(permission_pda);
    match permission_info {
        Some(info) => {
            if info.data.is_empty() {
                Ok(None)
            } else {
                Permission::try_from_slice(&info.data[1..])
                    .map(Some)
                    .map_err(Into::into)
            }
        }
        None => Ok(None),
    }
}

pub fn manage_permission(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    payer: &Keypair,
    calling_authority: &Keypair,
    subject_authority: &Pubkey,
    status: PermissionStatus,
    can_execute_swap: bool,
    can_manage_permissions: bool,
    can_invoke_external_transfer: bool,
    can_reallocate: bool,
    can_freeze_controller: bool,
    can_unfreeze_controller: bool,
    can_manage_reserves_and_integrations: bool,
    can_suspend_permissions: bool,
    can_liquidate: bool,
) -> Result<Pubkey, Box<dyn Error>> {
    let calling_permission_pda = derive_permission_pda(controller, &calling_authority.pubkey());
    let calling_permission_account_before = fetch_permission_account(svm, &calling_permission_pda)?;
    let controller_authority = derive_controller_authority_pda(controller);

    // Ensure the calling permission exists before the transaction
    assert!(
        calling_permission_account_before.is_some(),
        "Calling permission account must exist before the transaction"
    );

    let subject_permission_pda = derive_permission_pda(controller, subject_authority);

    let subject_permission_account_before = fetch_permission_account(svm, &subject_permission_pda)?;

    let ixn = ManagePermissionBuilder::new()
        .status(status)
        .can_execute_swap(can_execute_swap)
        .can_manage_permissions(can_manage_permissions)
        .can_invoke_external_transfer(can_invoke_external_transfer)
        .can_reallocate(can_reallocate)
        .can_freeze_controller(can_freeze_controller)
        .can_unfreeze_controller(can_unfreeze_controller)
        .can_manage_reserves_and_integrations(can_manage_reserves_and_integrations)
        .can_suspend_permissions(can_suspend_permissions)
        .can_liquidate(can_liquidate)
        .payer(payer.pubkey())
        .controller(*controller)
        .controller_authority(controller_authority)
        .super_authority(calling_authority.pubkey())
        .super_permission(calling_permission_pda)
        .authority(*subject_authority)
        .permission(subject_permission_pda)
        .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&calling_authority, &payer],
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn.clone());
    if tx_result.is_err() {
        println!("{:#?}", tx_result.clone().unwrap().logs);
    } else {
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    let calling_permission_account_after = fetch_permission_account(svm, &calling_permission_pda)?;
    let subject_permission_account_after = fetch_permission_account(svm, &subject_permission_pda)?;

    // Ensure both permission accounts exist after the transaction
    assert!(
        calling_permission_account_after.is_some(),
        "Calling permission account must exist after the transaction"
    );
    assert!(
        subject_permission_account_after.is_some(),
        "Subject permission account must exist after the transaction"
    );

    // If the calling and subject addresses are different, check that the calling values are unchanged
    if calling_authority.pubkey() != *subject_authority {
        let calling_permission_before = calling_permission_account_before.unwrap();
        let calling_permission_after = calling_permission_account_after.unwrap();
        assert_eq!(
            calling_permission_before, calling_permission_after,
            "Calling permission values have changed"
        );
    }

    // Check that the subject values, controller, and authority are aligned to the inputs
    let subject_permission_after = subject_permission_account_after.clone().unwrap();
    assert_eq!(
        subject_permission_after.controller, *controller,
        "Subject permission controller does not match the expected controller"
    );
    assert_eq!(
        subject_permission_after.authority, *subject_authority,
        "Subject permission authority does not match the expected authority"
    );
    assert_eq!(
        subject_permission_after.status, status,
        "Subject permission status does not match the expected status"
    );
    assert_eq!(
        subject_permission_after.can_execute_swap, can_execute_swap,
        "Subject permission to execute swap does not match the expected value"
    );
    assert_eq!(
        subject_permission_after.can_manage_permissions, can_manage_permissions,
        "Subject permission to manage permissions does not match the expected value"
    );
    assert_eq!(
        subject_permission_after.can_invoke_external_transfer, can_invoke_external_transfer,
        "Subject permission to invoke external transfer does not match the expected value"
    );
    assert_eq!(
        subject_permission_after.can_reallocate, can_reallocate,
        "Subject permission to reallocate does not match the expected value"
    );
    assert_eq!(
        subject_permission_after.can_freeze_controller, can_freeze_controller,
        "Subject permission to freeze does not match the expected value"
    );
    assert_eq!(
        subject_permission_after.can_unfreeze_controller, can_unfreeze_controller,
        "Subject permission to unfreeze does not match the expected value"
    );
    assert_eq!(
        subject_permission_after.can_manage_reserves_and_integrations,
        can_manage_reserves_and_integrations,
        "Subject permission to manage integrations does not match the expected value"
    );

    // assert expected event was emitted
    let expected_event = SvmAlmControllerEvent::PermissionUpdate(PermissionUpdateEvent {
        controller: *controller,
        permission: subject_permission_pda,
        authority: *subject_authority,
        old_state: subject_permission_account_before,
        new_state: subject_permission_account_after,
    });
    assert_contains_controller_cpi_event!(
        tx_result.unwrap(),
        txn.message.account_keys.as_slice(),
        expected_event
    );

    Ok(subject_permission_pda)
}
