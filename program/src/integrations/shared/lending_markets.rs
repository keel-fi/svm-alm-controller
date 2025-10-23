/* Shared logic across Lending integrations */
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{account_info::AccountInfo, pubkey::Pubkey, ProgramResult};
use shank::ShankType;

use crate::{
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    state::Controller,
};

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct LendingState {
    /// The balance of tokens that the Controller has a claim on.
    /// This includes the deposit amount plus any interest earned.
    pub balance: u64,
    /// Padding
    pub _padding: [u8; 40],
}

/// Emits an Integration Sync AccountingEvent when the balance
/// within the lending protocol changes (usually due to interest accrual).
pub fn emit_lending_balance_sync_event(
    controller: &Controller,
    integration_pubkey: &Pubkey,
    controller_pubkey: &Pubkey,
    controller_authority: &AccountInfo,
    mint: &Pubkey,
    prev_balance: u64,
    new_balance: u64,
) -> ProgramResult {
    if prev_balance == new_balance {
        return Ok(());
    }

    let abs_delta = new_balance.abs_diff(prev_balance);

    let direction = if new_balance > prev_balance {
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
            mint: *mint,
            action: AccountingAction::Sync,
            delta: abs_delta,
            direction,
        }),
    )
}
