use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    enums::IntegrationState,
    integrations::{
        drift::{balance::get_drift_lending_balance, protocol_state::SpotMarket},
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
    spot_market: &AccountInfo,
    user: &AccountInfo,
) -> Result<u64, ProgramError> {
    let balance = match &integration.state {
        IntegrationState::Drift(drift_state) => drift_state.balance,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let spot_market_data = spot_market.try_borrow_data()?;
    let spot_market_state = SpotMarket::try_from_slice(&spot_market_data)?;

    let new_balance = get_drift_lending_balance(&spot_market_state, user)?;

    emit_lending_balance_sync_event(
        controller,
        integration_pubkey,
        controller_pubkey,
        controller_authority,
        &spot_market_state.mint,
        balance,
        new_balance,
    )?;

    Ok(new_balance)
}
