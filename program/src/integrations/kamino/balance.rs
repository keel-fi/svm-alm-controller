use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{
    integrations::kamino::protocol_state::{KaminoReserve, Obligation},
    processor::shared::is_account_closed,
};

/// This function gets the Kamino lending balance (liquidity_value) from protocol state.
/// It handles the cases where:
///     - The `Obligation` has been closed (full withdrawal).
///     - The `ObligationCollateral` doesn't exist yet (first deposit or full withdrawal).
pub fn get_kamino_lending_balance(
    kamino_reserve: &AccountInfo,
    obligation: &AccountInfo,
) -> Result<u64, ProgramError> {
    // if the obligation is closed
    // (there has been a full withdrawal and it only had one ObligationCollateral slot used),
    // then the lp_amount is 0

    if is_account_closed(obligation) {
        return Ok(0);
    }

    // if it's not closed, then we read the state,
    // but its possible that the ObligationCollateral hasn't been created yet (first deposit)
    // in that case lp_amount is also 0
    let obligation_data = obligation.try_borrow_data()?;
    let obligation_state = Obligation::load_checked(&obligation_data)?;

    // handles the case where no ObligationCollateral is found
    let lp_amount = obligation_state
        .get_obligation_collateral_for_reserve(kamino_reserve.key())
        .map_or(0, |collateral| collateral.deposited_amount);

    // avoids deserializing kamino_reserve if lp_amount is 0
    if lp_amount == 0 {
        return Ok(0);
    }

    let kamino_reserve_data = kamino_reserve.try_borrow_data()?;
    let kamino_reserve_state = KaminoReserve::load_checked(&kamino_reserve_data)?;
    let liquidity_value = kamino_reserve_state.collateral_to_liquidity(lp_amount);

    Ok(liquidity_value)
}
