use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::ManagePermissionBuilder, types::PermissionStatus},
};
use solana_instruction::Instruction;
use solana_program::system_program;
use solana_pubkey::Pubkey;

/// Instruction generation for managing a permission account

pub fn create_manage_permissions_instruction(
    controller: &Pubkey,
    payer: &Pubkey,
    calling_authority: &Pubkey,
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
) -> Instruction {
    let calling_permission_pda = derive_permission_pda(controller, &calling_authority);
    let controller_authority = derive_controller_authority_pda(controller);

    let subject_permission_pda = derive_permission_pda(controller, subject_authority);

    ManagePermissionBuilder::new()
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
        .payer(*payer)
        .controller(*controller)
        .controller_authority(controller_authority)
        .super_authority(*calling_authority)
        .super_permission(calling_permission_pda)
        .authority(*subject_authority)
        .permission(subject_permission_pda)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction()
}
