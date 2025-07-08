use borsh::BorshDeserialize;
use litesvm::LiteSVM;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, transaction::Transaction,
};
use std::error::Error;
use svm_alm_controller_client::generated::{
    accounts::Controller,
    instructions::InitializeControllerBuilder,
    programs::SVM_ALM_CONTROLLER_ID,
    types::{ControllerStatus, PermissionStatus},
};

use crate::subs::{derive_permission_pda, fetch_permission_account};

pub fn derive_controller_pda(id: &u16) -> Pubkey {
    let (controller_pda, _controller_bump) = Pubkey::find_program_address(
        &[b"controller", &id.to_le_bytes()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    controller_pda
}

pub fn derive_controller_authority_pda(controller_pda: &Pubkey) -> Pubkey {
    let (controller_authority_pda, _controller_authority_bump) = Pubkey::find_program_address(
        &[b"controller_authority", controller_pda.as_ref()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    controller_authority_pda
}

pub fn fetch_controller_account(
    svm: &mut LiteSVM,
    controller_pda: &Pubkey,
) -> Result<Option<Controller>, Box<dyn Error>> {
    let controller_info = svm.get_account(controller_pda);
    match controller_info {
        Some(info) => {
            if info.data.is_empty() {
                Ok(None)
            } else {
                Controller::try_from_slice(&info.data[1..])
                    .map(Some)
                    .map_err(Into::into)
            }
        }
        None => Ok(None),
    }
}

pub fn initialize_contoller(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    status: ControllerStatus,
    id: u16,
) -> Result<(Pubkey, Pubkey), Box<dyn Error>> {
    let controller_pda = derive_controller_pda(&id);
    let controller_authority = derive_controller_authority_pda(&controller_pda);
    let permission_pda = derive_permission_pda(&controller_pda, &authority.pubkey());

    let ixn = InitializeControllerBuilder::new()
        .id(id)
        .status(status)
        .payer(payer.pubkey())
        .authority(authority.pubkey())
        .controller(controller_pda)
        .controller_authority(controller_authority)
        .permission(permission_pda)
        .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&authority, &payer],
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn);
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let controller = fetch_controller_account(svm, &controller_pda)?;
    let permission = fetch_permission_account(svm, &permission_pda)?;

    assert!(controller.is_some(), "Controller account is not found");
    let controller = controller.unwrap();
    assert_eq!(
        controller.status, status,
        "Controller status does not match the expected status"
    );
    assert_eq!(
        controller.id, id,
        "Controller ID does not match the expected ID"
    );

    assert!(permission.is_some(), "Permission account is not found");
    let permission = permission.unwrap();
    assert_eq!(
        permission.authority,
        authority.pubkey(),
        "Permission authority does not match the expected authority"
    );
    assert_eq!(
        permission.controller, controller_pda,
        "Permission controller does not match the expected controller PDA"
    );
    assert_eq!(
        permission.can_manage_permissions, true,
        "Permission to manage permissions is not set to true"
    );
    assert_eq!(
        permission.can_execute_swap, false,
        "Permission to execute swap is not set to false"
    );
    assert_eq!(
        permission.can_freeze, false,
        "Permission to freeze is not set to false"
    );
    assert_eq!(
        permission.can_unfreeze, false,
        "Permission to unfreeze is not set to false"
    );
    assert_eq!(
        permission.can_invoke_external_transfer, false,
        "Permission to invoke external transfer is not set to false"
    );
    assert_eq!(
        permission.can_manage_integrations, false,
        "Permission to manage integrations is not set to false"
    );
    assert_eq!(
        permission.can_reallocate, false,
        "Permission to reallocate is not set to false"
    );
    assert_eq!(
        permission.status,
        PermissionStatus::Active,
        "Permission status is not set to Active"
    );

    Ok((controller_pda, permission_pda))
}
