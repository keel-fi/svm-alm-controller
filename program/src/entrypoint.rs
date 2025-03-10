use pinocchio::{
    account_info::AccountInfo, entrypoint, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::processor::{
    process_emit_event, 
    process_initialize_controller, 
    process_initialize_integration, 
    process_manage_permission, 
    process_push, 
    process_sync
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
        3 => process_initialize_integration(program_id, accounts, instruction_data),
        4 => process_sync(program_id, accounts, instruction_data),
        5 => process_push(program_id, accounts, instruction_data),
        // Other methods
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
