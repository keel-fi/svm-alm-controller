use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{msg, program_error::ProgramError, pubkey::Pubkey};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct PsmSwapConfig {
    pub psm_token: Pubkey,
    pub psm_pool: Pubkey,
    pub mint: Pubkey,
    pub _padding: [u8; 128],
}

impl PsmSwapConfig {
    /// Validate the Integration instruction matches the config.
    /// This is to prevent an Integration instance from being used
    /// for unintended tokens/markets.
    pub fn check_accounts(
        &self,
        psm_token: &Pubkey,
        psm_pool: &Pubkey,
        mint: &Pubkey,
    ) -> Result<(), ProgramError> {
        if psm_token.ne(&self.psm_token) {
            msg!("psm_token: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        if psm_pool.ne(&self.psm_pool) {
            msg!("psm_pool: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        if mint.ne(&self.mint) {
            msg!("mint: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }
}
