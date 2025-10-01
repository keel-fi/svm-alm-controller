use crate::{
    derive_controller_authority_pda, derive_reserve_pda,
    generated::instructions::SyncReserveBuilder,
};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

/// Instruction generation for syncing a reserve account
pub fn create_sync_reserve_instruction(
    controller: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    let reserve_pda = derive_reserve_pda(controller, mint);
    let controller_authority = derive_controller_authority_pda(controller);
    let vault = get_associated_token_address_with_program_id(&controller_authority, mint, token_program);

    SyncReserveBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .reserve(reserve_pda)
        .vault(vault)
        .instruction()
}
