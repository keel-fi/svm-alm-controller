use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{msg, program_error::ProgramError, pubkey::Pubkey};
use shank::ShankType;

use crate::integrations::drift::pdas::derive_drift_user_pda;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct DriftConfig {
    // The sub account where borrow/lend are cross collateralized
    pub sub_account_id: u16,
    // Spot market to deposit into (mint specific)
    // Indexes can be seen here: https://github.com/drift-labs/protocol-v2/blob/master/sdk/src/constants/spotMarkets.ts
    pub spot_market_index: u16,
    pub _padding: [u8; 220],
}

impl DriftConfig {
    /// Validate the Integration instruction matches the config.
    /// This is to prevent an Integration instance from being used
    /// for unintended tokens/markets.
    pub fn check_accounts(
        &self,
        controller_authority: &Pubkey,
        drift_user: &Pubkey,
        spot_market_index: u16,
    ) -> Result<(), ProgramError> {
        let drift_user_pda = derive_drift_user_pda(controller_authority, self.sub_account_id)?;
        if drift_user_pda.ne(drift_user) {
            msg!("drift_user: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }
        if spot_market_index.ne(&self.spot_market_index) {
            msg!("spot_market_index: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}
