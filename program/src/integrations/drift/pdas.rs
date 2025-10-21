use pinocchio::{
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
};

use crate::integrations::drift::constants::DRIFT_PROGRAM_ID;

/// Derive the Drift User PDA from subaccount
pub fn derive_drift_user_pda(
    controller_authority: &Pubkey,
    sub_account_id: u16,
) -> Result<Pubkey, ProgramError> {
    let (pda, _) = try_find_program_address(
        &[
            b"user",
            controller_authority.as_ref(),
            &sub_account_id.to_le_bytes(),
        ],
        &DRIFT_PROGRAM_ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;
    Ok(pda)
}

/// Derive the Drift SpotMarket PDA
pub fn derive_drift_spot_market_pda(market_index: u16) -> Result<Pubkey, ProgramError> {
    let (pda, _) = try_find_program_address(
        &[b"spot_market", market_index.to_le_bytes().as_ref()],
        &DRIFT_PROGRAM_ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;
    Ok(pda)
}
