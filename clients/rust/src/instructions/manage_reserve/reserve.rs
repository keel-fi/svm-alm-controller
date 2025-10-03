use crate::{
    derive_controller_authority_pda, derive_permission_pda, derive_reserve_pda,
    generated::{instructions::ManageReserveBuilder, types::ReserveStatus},
};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

/// Instruction generation for managing a reserve account
pub fn create_manage_reserve_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    mint: &Pubkey,
    status: ReserveStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
) -> Instruction {
    let calling_permission_pda: Pubkey = derive_permission_pda(controller, authority);
    let reserve_pda = derive_reserve_pda(controller, mint);
    let controller_authority = derive_controller_authority_pda(controller);

    ManageReserveBuilder::new()
        .status(status)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .reserve(reserve_pda)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .instruction()
}
