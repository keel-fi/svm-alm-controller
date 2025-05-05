use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use crate::state::Controller;

pub fn process_emit_event(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("emit_cpi");

    let [authority_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let controller_id = instruction_data
        .get(0..2)
        .and_then(|s| s.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ProgramError::InvalidInstructionData)?;

    let (pda, _) = Controller::derive_pda_bytes(controller_id)?;

    // Validate the authority is the expected controller's PDA
    if authority_info.key().ne(&pda) {
        return Err(ProgramError::MissingRequiredSignature.into());
    }

    // The authority must be the signer
    if !authority_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    // The authority must be a PDA of this program
    if !authority_info.is_owned_by(&crate::ID) {
        return Err(ProgramError::InvalidAccountOwner.into());
    }

    Ok(())
}
