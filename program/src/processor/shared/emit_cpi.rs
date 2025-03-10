use borsh::BorshSerialize;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    ProgramResult,
};
use crate::{events::SvmAlmControllerEvent, instructions::EmitEventArgs};
use pinocchio::program::invoke_signed;
use crate::instructions::SvmAlmControllerInstruction;

/// Create a CPI to emit an event using the given authority and PDA signer seeds.
pub fn emit_cpi<const N: usize>(
    authority: &AccountInfo,
    signer_seeds: [Seed; N],
    event: SvmAlmControllerEvent
) -> ProgramResult {

    // Serialize the event data
    let serialized = event.try_to_vec().unwrap();

    // Prepare the instruction data and serialize it
    let instruction_data = SvmAlmControllerInstruction::EmitEvent(
        EmitEventArgs { data: serialized }
    ).try_to_vec().unwrap();
    
    // Create the instruction
    let instruction = Instruction {
        program_id: &crate::ID,
        accounts: &[
            AccountMeta::new(authority.key(), false, true),
        ],
        data: instruction_data.as_slice(),
    };
    
    // Invoke the instruction with the PDA as a signer
    let signer = Signer::from(&signer_seeds);
    invoke_signed(
        &instruction,
        &[authority],
        &[signer]
    )?;
    
    Ok(())
}
