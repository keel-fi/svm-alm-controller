use crate::{derive_controller_authority_pda, generated::instructions::SyncBuilder, SVM_ALM_CONTROLLER_ID};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

pub fn create_sync_integration_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);

    SyncBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .integration(*integration)
        .reserve(*reserve)
        .program_id(SVM_ALM_CONTROLLER_ID)
        .instruction()
}
