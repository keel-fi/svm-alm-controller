use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    enums::IntegrationState,
    integrations::{
        kamino::protocol_state::get_liquidity_amount,
        shared::lending_markets::emit_lending_balance_sync_event,
    },
    state::{Controller, Integration},
};

/// Calculates the current `liquidity_value` of `position`/`kamino_reserve` and emits Sync event
/// in the case of a change regarding previously stored `liquidity_value`.
/// Used in Push/Pull/Sync.
pub fn sync_kamino_liquidity_value(
    controller: &Controller,
    integration: &Integration,
    integration_pubkey: &Pubkey,
    controller_pubkey: &Pubkey,
    controller_authority: &AccountInfo,
    liquidity_mint: &Pubkey,
    kamino_reserve: &AccountInfo,
    obligation: &AccountInfo,
) -> Result<u64, ProgramError> {
    let last_liquidity_value = match &integration.state {
        IntegrationState::Kamino(state) => state.balance,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let new_liquidity_value = get_liquidity_amount(kamino_reserve, obligation)?;

    emit_lending_balance_sync_event(
        controller,
        integration_pubkey,
        controller_pubkey,
        controller_authority,
        liquidity_mint,
        last_liquidity_value,
        new_liquidity_value,
    )?;

    Ok(new_liquidity_value)
}
