use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::ManageControllerBuilder, types::ControllerStatus},
};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;


pub fn create_manage_controller_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    status: ControllerStatus,
) -> Instruction {
    let calling_permission_pda = derive_permission_pda(controller, &authority);
    let controller_authority = derive_controller_authority_pda(controller);

    ManageControllerBuilder::new()
        .status(status)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .instruction()
}