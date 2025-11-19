use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_token_interface::TokenAccount;

use crate::{
    enums::IntegrationState,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    state::{Controller, Integration},
};

pub fn sync_psm_liquidity_supplied(
    controller: &Controller,
    integration: &Integration,
    controller_pubkey: &Pubkey,
    integration_pubkey: &Pubkey,
    mint_pubkey: &Pubkey,
    controller_authority: &AccountInfo,
    psm_token_vault: &AccountInfo,
) -> Result<u64, ProgramError> {
    let prev_liquidity_supplied = match &integration.state {
        IntegrationState::PsmSwap(state) => state.liquidity_supplied,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let new_vault_liquidity = TokenAccount::from_account_info(psm_token_vault)?.amount();

    let liquidity_delta = new_vault_liquidity.abs_diff(prev_liquidity_supplied);

    if liquidity_delta > 0 {
        let direction = if new_vault_liquidity > prev_liquidity_supplied {
            // liquidity increased
            AccountingDirection::Credit
        } else {
            // liquidity decreased
            AccountingDirection::Debit
        };

        controller.emit_event(
            controller_authority,
            controller_pubkey,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *controller_pubkey,
                integration: Some(*integration_pubkey),
                reserve: None,
                mint: *mint_pubkey,
                action: AccountingAction::Sync,
                delta: liquidity_delta,
                direction,
            }),
        )?;
    }

    Ok(new_vault_liquidity)
}
