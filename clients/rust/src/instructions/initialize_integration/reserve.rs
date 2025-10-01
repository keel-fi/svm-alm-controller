use crate::{
    derive_controller_authority_pda, derive_permission_pda, derive_reserve_pda,
    generated::{instructions::InitializeReserveBuilder, types::ReserveStatus},
};
use pinocchio_associated_token_account::ID as PINOCCHIO_ASSOCIATED_TOKEN_ACCOUNT_ID;
use solana_instruction::Instruction;
use solana_program::system_program;
use solana_pubkey::Pubkey;
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

/// Instruction generation for initializing a reserve account
pub fn create_initialize_reserve_instruction(
    payer: &Pubkey,
    controller: &Pubkey,
    authority: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
    status: ReserveStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
) -> Instruction {
    let calling_permission_pda: Pubkey = derive_permission_pda(controller, &authority);

    let reserve_pda = derive_reserve_pda(controller, mint);

    let controller_authority = derive_controller_authority_pda(controller);

    let vault =
        get_associated_token_address_with_program_id(&controller_authority, mint, token_program);

    InitializeReserveBuilder::new()
        .status(status)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .payer(*payer)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .reserve(reserve_pda)
        .mint(*mint)
        .vault(vault)
        .token_program(*token_program)
        .associated_token_program(PINOCCHIO_ASSOCIATED_TOKEN_ACCOUNT_ID.into())
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction()
}
