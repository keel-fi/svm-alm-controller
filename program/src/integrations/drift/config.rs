use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{msg, program_error::ProgramError};
use shank::ShankType;

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
    pub fn check_accounts(
        &self,
        sub_account_id: u16,
        spot_market_index: u16,
    ) -> Result<(), ProgramError> {
        if sub_account_id != self.sub_account_id {
            msg!("sub_account_id: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }
        if spot_market_index != self.spot_market_index {
            msg!("spot_market_index: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}
