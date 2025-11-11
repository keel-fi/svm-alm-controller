use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::integrations::drift::{pdas::derive_drift_spot_market_pda, protocol_state::SpotMarket};

/// Find the Drift SpotMarket and Oracle accounts in `remaining_accounts` by market index.
pub fn find_spot_market_and_oracle_account_info_by_id<'info>(
    account_infos: &'info [AccountInfo],
    market_index: u16,
) -> Result<(&'info AccountInfo, &'info AccountInfo), ProgramError> {
    // Derive SpotMarket Pubkey given the index
    let spot_market_pubkey = derive_drift_spot_market_pda(market_index)?;
    // Get SpotMarket AccountInfo and deserialize
    let spot_market_info = account_infos
        .iter()
        .find(|acct| acct.key().eq(&spot_market_pubkey))
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    let spot_market_data = spot_market_info.try_borrow_data()?;
    let spot_market = SpotMarket::try_from_slice(&spot_market_data)?;

    // Find Oracle account for the SpotMarket
    let oracle_info = account_infos
        .iter()
        .find(|acct| acct.key().eq(&spot_market.oracle))
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    Ok((spot_market_info, oracle_info))
}
