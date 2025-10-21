use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::integrations::drift::pdas::derive_drift_spot_market_pda;

/// Find the Drift SpotMarket account in `remaining_accounts` by market index
pub fn find_spot_market_account_info_by_id<'info>(
    account_infos: &'info [AccountInfo],
    market_index: u16,
) -> Result<&'info AccountInfo, ProgramError> {
    let spot_market_pubkey = derive_drift_spot_market_pda(market_index)?;
    account_infos
        .iter()
        .find(|acct| acct.key().eq(&spot_market_pubkey))
        .ok_or(ProgramError::NotEnoughAccountKeys)
}
