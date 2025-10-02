use crate::{
    generated::instructions::SyncBuilder,
};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

pub fn create_sync_integration_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    integration: &Pubkey,
) -> Instruction {
    SyncBuilder::new()
        .controller(*controller)
        .controller_authority(*authority)
        .integration(*integration)
        .instruction()
}