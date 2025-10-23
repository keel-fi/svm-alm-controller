use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    enums::IntegrationState,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    integrations::kamino::protocol_state::get_liquidity_and_lp_amount,
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
) -> Result<(u64, u64), ProgramError> {
    let last_liquidity_value = match &integration.state {
        IntegrationState::Kamino(state) => state.balance,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let (new_liquidity_value, new_lp_amount) =
        get_liquidity_and_lp_amount(kamino_reserve, obligation)?;

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

    Ok((new_liquidity_value, new_lp_amount))
}
