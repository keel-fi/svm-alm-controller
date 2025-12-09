use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct PsmSwapConfig {
    pub psm_token: Pubkey,
    pub psm_pool: Pubkey,
    pub mint: Pubkey,
    pub _padding: [u8; 128],
}

impl PsmSwapConfig {
    /// Validates config matches the provided AccountInfos pubkeys
    pub fn check_accounts(
        &self,
        psm_token: &AccountInfo,
        psm_pool: &AccountInfo,
        mint: &AccountInfo,
    ) -> Result<(), ProgramError> {
        if self.psm_pool.ne(psm_pool.key()) {
            msg!("psm_pool: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        if self.psm_token.ne(psm_token.key()) {
            msg!("psm_token: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        if self.mint.ne(mint.key()) {
            msg!("mint: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }
}
