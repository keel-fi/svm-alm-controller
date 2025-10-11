use crate::{
    solana_utils::{try_find_program_address, ProgramError, Pubkey},
    DRIFT_PROGRAM_ID,
};

pub fn derive_user_stats_pda(authority: &Pubkey) -> Result<Pubkey, ProgramError> {
    let (pubkey, _bump) =
        try_find_program_address(&[b"user_stats", authority.as_ref()], &DRIFT_PROGRAM_ID)
            .ok_or(ProgramError::InvalidSeeds)?;
    Ok(pubkey)
}
