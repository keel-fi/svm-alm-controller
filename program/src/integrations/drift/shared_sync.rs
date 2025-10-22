use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_log::log;

use crate::{
    enums::IntegrationState,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    integrations::drift::protocol_state::{SpotMarket, User},
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
    liquidity_mint: &Pubkey,
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

    let (scaled_balance, balance_type) = if let Some(pos) = spot_position {
        (pos.scaled_balance, pos.balance_type)
    } else {
        // If the spot position is not found, return 0 bc it doesn't exist yet
        (0, 0)
    };

    let new_balance = spot_market_state.get_token_amount(scaled_balance as u128, balance_type)?;

    if balance != new_balance {
        let abs_delta = new_balance.abs_diff(balance);

        let direction = if new_balance > balance {
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

    Ok(new_balance)
}
