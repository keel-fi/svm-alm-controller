use pinocchio::{
    account_info::AccountInfo, 
    program_error::ProgramError, pubkey::Pubkey
};

use crate::{
    enums::IntegrationState, 
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent}, 
    integrations::utilization_market::{
        kamino::kamino_state::get_liquidity_and_lp_amount, 
        state::UtilizationMarketState
    }, 
    state::{Controller, Integration}
};

/// Calculates the current `liquidity_value` of `position`/`kamino_reserve` and emits Sync event
/// in the case of a change regarding previously stores `liquidity_value`.
/// Used in Push/Pull/Sync.
pub fn sync_kamino_liquidity_value(
    controller: &Controller,
    integration: &Integration,
    integration_pubkey: &Pubkey,
    controller_pubkey: &Pubkey,
    controller_authority: &AccountInfo,
    liquidity_mint: &Pubkey,
    kamino_reserve: &AccountInfo,
    obligation: &AccountInfo
) -> Result<(u64, u64), ProgramError> {
    let last_liquidity_value = match &integration.state {
        IntegrationState::UtilizationMarket(s) => match s {
            UtilizationMarketState::KaminoState(state) => state.last_liquidity_value
        },
        _ => return Err(ProgramError::InvalidAccountData)
    };

    let (new_liquidity_value, new_lp_amount) = get_liquidity_and_lp_amount(
        kamino_reserve, 
        obligation, 
    )?;

    if last_liquidity_value != new_liquidity_value {
        controller.emit_event(
            controller_authority, 
            controller_pubkey, 
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent { 
                controller: *controller_pubkey, 
                integration: *integration_pubkey, 
                mint: *liquidity_mint, 
                action: AccountingAction::Sync, 
                before: last_liquidity_value, 
                after: new_liquidity_value
            })
        )?
    }

    Ok((new_liquidity_value, new_lp_amount))
}