use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::integrations::drift::protocol_state::{SpotMarket, User};

/// Calculate the current balance of a Drift spot position.
/// This is used to update Integration state and accounts for
/// accrued interest.
pub fn get_drift_lending_balance(
    spot_market: &AccountInfo,
    user: &AccountInfo,
) -> Result<u64, ProgramError> {
    let spot_market_data = spot_market.try_borrow_data()?;
    let spot_market_state = SpotMarket::try_from_slice(&spot_market_data)?;

    let user_data = user.try_borrow_data()?;
    let user_state = User::try_from_slice(&user_data)?;

    let spot_position = user_state
        .spot_positions
        .iter()
        .find(|pos| pos.market_index == spot_market_state.market_index);

    let new_balance = if let Some(pos) = spot_position {
        spot_market_state.get_token_amount(pos.scaled_balance as u128, pos.balance_type)?
    } else {
        0
    };

    Ok(new_balance)
}
