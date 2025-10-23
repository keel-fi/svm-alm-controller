use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    enums::IntegrationState,
    integrations::{
        drift::protocol_state::{SpotMarket, User},
        shared::lending_markets::emit_lending_balance_sync_event,
    },
    state::{Controller, Integration},
};

/// Calculates the current `balance` of a Drift spot position and emits Sync event
/// in the case of a change regarding previously stored `balance`.
pub fn sync_drift_balance(
    controller: &Controller,
    integration: &Integration,
    integration_pubkey: &Pubkey,
    controller_pubkey: &Pubkey,
    controller_authority: &AccountInfo,
    mint: &Pubkey,
    spot_market: &AccountInfo,
    user: &AccountInfo,
    market_index: u16,
) -> Result<u64, ProgramError> {
    let balance = match &integration.state {
        IntegrationState::Drift(drift_state) => drift_state.balance,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let spot_market_data = spot_market.try_borrow_data()?;
    let spot_market_state = SpotMarket::load_checked(&spot_market_data)?;

    let user_data = user.try_borrow_data()?;
    let user_state = User::try_from(&user_data)?;

    let spot_position = user_state
        .spot_positions
        .iter()
        .find(|pos| pos.market_index == market_index);

    let new_balance = if let Some(pos) = spot_position {
        spot_market_state.get_token_amount(pos.scaled_balance as u128, pos.balance_type)?
    } else {
        0
    };

    emit_lending_balance_sync_event(
        controller,
        integration_pubkey,
        controller_pubkey,
        controller_authority,
        mint,
        balance,
        new_balance,
    )?;

    Ok(new_balance)
}
