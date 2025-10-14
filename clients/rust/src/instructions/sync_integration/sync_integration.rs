use crate::{derive_controller_authority_pda, generated::instructions::SyncBuilder};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

pub fn create_sync_integration_instruction(
    controller: &Pubkey,
    payer: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);

    SyncBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .payer(*payer)
        .integration(*integration)
        .reserve(*reserve)
        .instruction()
}
