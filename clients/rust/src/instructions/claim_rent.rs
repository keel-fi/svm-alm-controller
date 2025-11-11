use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::instructions::ClaimRentBuilder,
};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use solana_sdk::system_program;

pub fn create_claim_rent_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    destination: &Pubkey,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);
    let permission = derive_permission_pda(controller, authority);

    ClaimRentBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(permission)
        .destination(*destination)
        .system_program(system_program::ID)
        .instruction()
}
