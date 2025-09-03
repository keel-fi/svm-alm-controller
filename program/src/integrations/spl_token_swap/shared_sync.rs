use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_token_interface::{Mint, TokenAccount};

use crate::{
    enums::IntegrationState,
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    state::{Controller, Integration},
};

/// Calculates the prorated balance of a pool based on the LP token balance
pub fn calculate_prorated_balance(pool_amount: u64, lp_balance: u64, lp_total_supply: u64) -> u64 {
    let res = u128::from(pool_amount)
        .checked_mul(lp_balance as u128)
        .expect("overflow")
        .saturating_div(lp_total_supply as u128);
    u64::try_from(res).expect("overflow")
}

/// Calculates the updated balances for the SPL Token Swap integration
/// and emits accounting events for the changes in balances.
///
/// This is intended to be shared across the Push|Pull|Sync instructions.
pub fn sync_spl_token_swap_integration(
    controller: &Controller,
    integration: &mut Integration,
    controller_acct: &AccountInfo,
    controller_authority: &AccountInfo,
    integration_acct: &AccountInfo,
    swap_token_a: &AccountInfo,
    swap_token_b: &AccountInfo,
    lp_token_acct: &AccountInfo,
    lp_mint_acct: &AccountInfo,
    mint_a_pubkey: &Pubkey,
    mint_b_pubkey: &Pubkey,
) -> Result<(u64, u64, u64), ProgramError> {
    let lp_mint = Mint::from_account_info(lp_mint_acct)?;
    let lp_mint_supply = lp_mint.supply();

    // Extract the values from the last update
    let (last_balance_a, last_balance_b, last_balance_lp) = match integration.state {
        IntegrationState::SplTokenSwap(state) => (
            state.last_balance_a,
            state.last_balance_b,
            state.last_balance_lp,
        ),
        _ => return Err(ProgramError::InvalidAccountData),
    };

    // STEP 1: Get the changes due to relative movement between token A and B
    // LP tokens constant, relative balance of A and B changed
    // (based on the old number of lp tokens)

    let swap_token_a = TokenAccount::from_account_info(swap_token_a)?;
    let swap_token_b = TokenAccount::from_account_info(swap_token_b)?;
    let pool_balance_a = swap_token_a.amount();
    let pool_balance_b = swap_token_b.amount();

    let step_1_balance_a: u64;
    let step_1_balance_b: u64;
    if last_balance_lp > 0 {
        step_1_balance_a =
            calculate_prorated_balance(pool_balance_a, last_balance_lp, lp_mint_supply);
        step_1_balance_b =
            calculate_prorated_balance(pool_balance_b, last_balance_lp, lp_mint_supply);
    } else {
        step_1_balance_a = 0u64;
        step_1_balance_b = 0u64;
    }
    // Emit the accounting events for the change in A and B's relative balances
    if last_balance_a != step_1_balance_a {
        controller.emit_event(
            controller_authority,
            controller_acct.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *controller_acct.key(),
                integration: *integration_acct.key(),
                mint: *mint_a_pubkey,
                action: AccountingAction::Sync,
                before: last_balance_a,
                after: step_1_balance_a,
            }),
        )?;
    }
    if last_balance_b != step_1_balance_b {
        controller.emit_event(
            controller_authority,
            controller_acct.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *controller_acct.key(),
                integration: *integration_acct.key(),
                mint: *mint_b_pubkey,
                action: AccountingAction::Sync,
                before: last_balance_b,
                after: step_1_balance_b,
            }),
        )?;
    }

    // Load in the vault, since it could have an opening balance
    let lp_token_account = TokenAccount::from_account_info(lp_token_acct)?;
    let new_balance_lp = lp_token_account.amount();

    // STEP 2: If the number of LP tokens changed
    // We need to account for the change in our claim
    //  on the underlying A and B tokens as a result of this
    //  change in LP tokens

    let step_2_balance_a: u64;
    let step_2_balance_b: u64;
    if new_balance_lp != last_balance_lp {
        if new_balance_lp > 0 {
            step_2_balance_a =
                calculate_prorated_balance(pool_balance_a, new_balance_lp, lp_mint_supply);
            step_2_balance_b =
                calculate_prorated_balance(pool_balance_b, new_balance_lp, lp_mint_supply);
        } else {
            step_2_balance_a = 0u64;
            step_2_balance_b = 0u64;
        }
        // Emit the accounting events for the change in A and B's relative balances
        controller.emit_event(
            controller_authority,
            controller_acct.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *controller_acct.key(),
                integration: *integration_acct.key(),
                mint: *mint_a_pubkey,
                action: AccountingAction::Sync,
                before: step_1_balance_a,
                after: step_2_balance_a,
            }),
        )?;
        controller.emit_event(
            controller_authority,
            controller_acct.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *controller_acct.key(),
                integration: *integration_acct.key(),
                mint: *mint_b_pubkey,
                action: AccountingAction::Sync,
                before: step_1_balance_b,
                after: step_2_balance_b,
            }),
        )?;
    } else {
        // No change
        step_2_balance_a = step_1_balance_a;
        step_2_balance_b = step_1_balance_b;
    }

    Ok((step_2_balance_a, step_2_balance_b, new_balance_lp))
}
