use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};


pub fn process_emit_event(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("emit_cpi");

    let [authority_info] = accounts else { return Err(ProgramError::NotEnoughAccountKeys) };
    // The authority must be the signer
    if !authority_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    // The authority must be a PDA of this program
    if authority_info.owner() != &crate::ID {
        return Err(ProgramError::InvalidAccountOwner.into());
    }
    
    Ok(())
}

