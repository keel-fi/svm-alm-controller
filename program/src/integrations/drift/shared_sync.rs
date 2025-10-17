use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    enums::IntegrationState,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    integrations::drift::protocol_state::{SpotMarket, User},
    state::{Controller, Integration},
};

/// Calculates the current `liquidity_value` of a Drift spot position and emits Sync event
/// in the case of a change regarding previously stored `liquidity_value`.
/// Used in Push/Pull/Sync.
pub fn sync_drift_liquidity_value(
    controller: &Controller,
    integration: &Integration,
    integration_pubkey: &Pubkey,
    controller_pubkey: &Pubkey,
    controller_authority: &AccountInfo,
    liquidity_mint: &Pubkey,
    spot_market: &AccountInfo,
    user: &AccountInfo,
    market_index: u16,
) -> Result<(u64, u64), ProgramError> {
    let last_liquidity_value = match &integration.state {
        IntegrationState::Drift(drift_state) => drift_state.last_liquidity_value,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let (new_liquidity_value, new_deposit_amount) =
        get_drift_liquidity_and_deposit_amount(spot_market, user, market_index)?;

    if last_liquidity_value != new_liquidity_value {
        let abs_delta = new_liquidity_value.abs_diff(last_liquidity_value);

        let direction = if new_liquidity_value > last_liquidity_value {
            // value increased
            AccountingDirection::Credit
        } else {
            // value decreased
            AccountingDirection::Debit
        };

        controller.emit_event(
            controller_authority,
            controller_pubkey,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *controller_pubkey,
                integration: Some(*integration_pubkey),
                reserve: None,
                mint: *liquidity_mint,
                action: AccountingAction::Sync,
                delta: abs_delta,
                direction,
            }),
        )?
    }

    Ok((new_liquidity_value, new_deposit_amount))
}

/// Calculate the liquidity value and deposit amount for a Drift spot position
fn get_drift_liquidity_and_deposit_amount(
    spot_market: &AccountInfo,
    user: &AccountInfo,
    market_index: u16,
) -> Result<(u64, u64), ProgramError> {
    // Load spot market data
    let spot_market_data = spot_market.try_borrow_data()?;
    let spot_market_state = SpotMarket::load_checked(&spot_market_data)?;

    // Load user data
    let user_data = user.try_borrow_data()?;
    let user_state = User::try_from(&user_data)?;

    // Find the spot position for the given market index
    let spot_position = user_state
        .spot_positions
        .iter()
        .find(|pos| pos.market_index == market_index && pos.balance_type == 0) // 0 = Deposit
        .ok_or(ProgramError::InvalidAccountData)?;

    // Calculate deposit amount: scaled_balance * cumulative_deposit_interest / SPOT_BALANCE_PRECISION
    // Note: This is a simplified calculation. In practice, you might need to handle precision more carefully
    let deposit_amount = (spot_position.scaled_balance as u128)
        .checked_mul(spot_market_state.cumulative_deposit_interest)
        .and_then(|result| result.checked_div(1_000_000_000)) // SPOT_BALANCE_PRECISION
        .and_then(|result| u64::try_from(result).ok())
        .ok_or(ProgramError::ArithmeticOverflow)?;

    // For liquidity value, we use the deposit amount as the base value
    // In a real implementation, you might want to apply additional calculations
    // such as applying asset weights or converting to quote asset value
    let liquidity_value = deposit_amount;

    Ok((liquidity_value, deposit_amount))
}
