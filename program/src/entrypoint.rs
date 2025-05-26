use pinocchio::{
    account_info::AccountInfo, entrypoint, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::processor::{
    process_atomic_swap_borrow, process_close_atomic_swap, process_emit_event,
    process_initialize_controller, process_initialize_integration, process_initialize_oracle,
    process_initialize_reserve, process_manage_integration, process_manage_permission,
    process_manage_reserve, process_pull, process_push, process_refresh_oracle,
    process_sync_integration, process_sync_reserve, process_update_oracle,
};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (discriminator, instruction_data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match discriminator {
        0 => process_emit_event(program_id, accounts, instruction_data),
        1 => process_initialize_controller(program_id, accounts, instruction_data),
        2 => process_manage_permission(program_id, accounts, instruction_data),
        3 => process_initialize_reserve(program_id, accounts, instruction_data),
        4 => process_manage_reserve(program_id, accounts, instruction_data),
        5 => process_initialize_integration(program_id, accounts, instruction_data),
        6 => process_manage_integration(program_id, accounts, instruction_data),
        7 => process_sync_reserve(program_id, accounts, instruction_data),
        8 => process_sync_integration(program_id, accounts, instruction_data),
        9 => process_push(program_id, accounts, instruction_data),
        10 => process_pull(program_id, accounts, instruction_data),
        11 => process_initialize_oracle(program_id, accounts, instruction_data),
        12 => process_update_oracle(program_id, accounts, instruction_data),
        13 => process_refresh_oracle(program_id, accounts),
        14 => process_close_atomic_swap(program_id, accounts),
        15 => process_atomic_swap_borrow(program_id, accounts, instruction_data),
        // Other methods
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
