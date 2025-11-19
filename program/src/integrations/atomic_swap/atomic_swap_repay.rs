use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token_interface::{instructions::TransferChecked, Mint, TokenAccount};

use crate::{
    constants::BPS_DENOMINATOR,
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    math::CheckedCeilDiv,
    state::{keel_account::KeelAccount, Controller, Integration, Oracle, Permission, Reserve},
};

define_account_struct! {
    pub struct AtomicSwapRepay<'info> {
        payer: signer;
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission: @owner(crate::ID);
        integration: mut, @owner(crate::ID);
        reserve_a: mut, @owner(crate::ID);
        vault_a: mut, @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        mint_a: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_b: mut, @owner(crate::ID);
        vault_b: mut, @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        mint_b:@owner(pinocchio_token::ID, pinocchio_token2022::ID);
        oracle: @owner(crate::ID);
        payer_account_a: mut, @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        payer_account_b: mut, @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program_a: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program_b: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
    }
}

pub fn process_atomic_swap_repay(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("atomic_swap_repay");
    let ctx = AtomicSwapRepay::from_accounts(accounts)?;
    let clock = Clock::get()?;

    // Load in the permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that authority has permission and the permission is active
    if !permission.can_execute_swap() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Load Controller for event emission.
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;

    // Check that mint and vault account matches known keys in controller-associated Reserve.
    let mut reserve_a = Reserve::load_and_check(ctx.reserve_a, ctx.controller.key())?;
    if reserve_a.vault != *ctx.vault_a.key() || reserve_a.mint.ne(ctx.mint_a.key()) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    let mut reserve_b = Reserve::load_and_check(ctx.reserve_b, ctx.controller.key())?;
    if reserve_b.vault != *ctx.vault_b.key() || reserve_b.mint.ne(ctx.mint_b.key()) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }

    // Check that Integration account is valid and matches controller.
    let mut integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;

    // Check that the Integration is of type AtomicSwap and has a valid config/state.
    let config = match &integration.config {
        IntegrationConfig::AtomicSwap(cfg) => cfg,
        _ => return Err(SvmAlmControllerErrors::Invalid.into()),
    };
    let state = match &mut integration.state {
        IntegrationState::AtomicSwap(state) => state,
        _ => return Err(SvmAlmControllerErrors::Invalid.into()),
    };
    let vault_a_swap_starting_balance = state.last_balance_a;
    let vault_b_swap_starting_balance = state.last_balance_b;

    // Validate config matches account and reserve state.
    if config.input_token.ne(&reserve_a.mint)
        || config.output_token.ne(&reserve_b.mint)
        || config.oracle.ne(ctx.oracle.key())
    {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }

    // Error if the swap has not started (aka no AtomicBorrow).
    if !state.has_swap_started() {
        return Err(SvmAlmControllerErrors::SwapNotStarted.into());
    }

    // Check that vault_a and vault_b amounts remain same as after atomic borrow.
    let vault_a = TokenAccount::from_account_info(ctx.vault_a)?;
    let vault_a_balance_before = vault_a.amount();
    let vault_b = TokenAccount::from_account_info(ctx.vault_b)?;
    let vault_b_balance_before = vault_b.amount();

    // Calculate the amount of token A/B the user has before repayment.
    let payer_account_a = TokenAccount::from_account_info(ctx.payer_account_a)?;
    // No need to error if the user overspent and has less tokens than the borrowed amount.
    // Amount over the users previous balance that still exists.
    let excess_token_a = payer_account_a
        .amount()
        .saturating_sub(state.recipient_token_a_pre);
    let payer_account_b = TokenAccount::from_account_info(ctx.payer_account_b)?;
    // Amount of Token B that the user accumulated between borrow & repay stages.
    let amount = payer_account_b
        .amount()
        .checked_sub(state.recipient_token_b_pre)
        .unwrap();

    // drop after reading amounts.
    drop(vault_a);
    drop(vault_b);
    drop(payer_account_a);
    drop(payer_account_b);

    // Check that vault_a and vault_b balances are not modified between atomic borrow and repay.
    if vault_a_balance_before
        .checked_add(state.amount_borrowed)
        .unwrap()
        != vault_a_swap_starting_balance
        || vault_b_balance_before != vault_b_swap_starting_balance
    {
        return Err(SvmAlmControllerErrors::InvalidSwapState.into());
    }

    // Transfer tokens to vault for repayment.
    let (final_input_amount, balance_a_delta) = if excess_token_a > 0 {
        let mint_a = Mint::from_account_info(ctx.mint_a)?;
        TransferChecked {
            from: ctx.payer_account_a,
            to: ctx.vault_a,
            mint: ctx.mint_a,
            authority: ctx.payer,
            amount: excess_token_a,
            decimals: mint_a.decimals(),
            token_program: ctx.token_program_a.key(),
        }
        .invoke()?;
        let balance_after = TokenAccount::from_account_info(ctx.vault_a)?.amount();
        // Calculate the amount that was received by the Reserve. This accounts for
        // a Transfer that has TransferFees enabled.
        let _balance_a_delta = balance_after
            .checked_sub(vault_a_balance_before)
            .expect("overflow");
        // Calculate the final amount the user spent from the Vault.
        // Saturating sub used in the ~unlikely~ event the change in balance is
        // greater than the amount borrowed.
        let _final_input_amount = state.amount_borrowed.saturating_sub(_balance_a_delta);
        (_final_input_amount, _balance_a_delta)
    } else {
        // No excess token A, so use the full amount borrowed and 0 for balance change since borrow.
        (state.amount_borrowed, 0)
    };

    let mint_b = Mint::from_account_info(ctx.mint_b)?;
    TransferChecked {
        from: ctx.payer_account_b,
        to: ctx.vault_b,
        mint: ctx.mint_b,
        authority: ctx.payer,
        amount,
        decimals: mint_b.decimals(),
        token_program: ctx.token_program_b.key(),
    }
    .invoke()?;
    let final_vault_balance_b = TokenAccount::from_account_info(ctx.vault_b)?.amount();
    // Calculate the amount that was received by the Reserve. This accounts for
    // a Transfer that has TransferFees enabled.
    let balance_b_delta = final_vault_balance_b
        .checked_sub(vault_b_balance_before)
        .expect("overflow");

    let oracle = Oracle::load_and_check(ctx.oracle, Some(ctx.controller.key()), None)?;

    // Check that oracle was last refreshed within acceptable staleness.
    if oracle.last_update_slot < clock.slot - config.max_staleness {
        return Err(SvmAlmControllerErrors::StaleOraclePrice.into());
    }

    // Check that swap is within accepted slippage of oracle price.
    check_swap_slippage(
        final_input_amount,
        config.input_mint_decimals,
        balance_b_delta,
        config.output_mint_decimals,
        config.max_slippage_bps,
        oracle.get_price(config.oracle_price_inverted),
        oracle.precision,
    )?;

    // Reset state after repayment.
    state.reset();

    // Update for rate limits and save.
    reserve_a.update_for_inflow(clock, balance_a_delta)?;
    reserve_a.save(ctx.reserve_a)?;
    reserve_b.update_for_inflow(clock, balance_b_delta)?;
    reserve_b.save(ctx.reserve_b)?;

    // Credit the Integration with the amount of Token A repaid.
    integration.update_rate_limit_for_inflow(clock, balance_a_delta)?;
    integration.save(ctx.integration)?;

    // Emit debit event for token a Reserve
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *ctx.controller.key(),
            integration: None,
            reserve: Some(*ctx.reserve_a.key()),
            mint: *ctx.mint_a.key(),
            action: AccountingAction::Swap,
            delta: final_input_amount,
            direction: AccountingDirection::Debit,
        }),
    )?;

    // Emit credit event for token b Reserve
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *ctx.controller.key(),
            integration: None,
            reserve: Some(*ctx.reserve_b.key()),
            mint: *ctx.mint_b.key(),
            action: AccountingAction::Swap,
            delta: balance_b_delta,
            direction: AccountingDirection::Credit,
        }),
    )?;

    Ok(())
}

fn pow10(decimals: u32) -> Option<i128> {
    10_i128.checked_pow(decimals)
}

fn calc_swap_price(
    in_factor: i128,
    out_factor: i128,
    prec_factor: i128,
    output_amount: i128,
    input_amount: i128,
) -> Result<i128, ProgramError> {
    // swap_price = (output_amount / out_factor) / (input_amount / in_factor) * prec_factor
    //            = (output_amount * in_factor * prec_factor) / (input_amount * out_factor)

    // Splitting numerator computation into 2 steps to avoid overflow while ensuring max retention
    // of precision.
    let step1 = output_amount.checked_mul(in_factor).unwrap();
    let step2 = step1.checked_mul(prec_factor);

    if let Some(numerator) = step2 {
        Ok(numerator
            .checked_div(input_amount)
            .unwrap()
            .checked_div(out_factor)
            .unwrap())
    } else {
        Ok(step1
            .checked_div(out_factor)
            .unwrap()
            .checked_mul(prec_factor)
            .unwrap()
            .checked_div(input_amount)
            .unwrap())
    }
}

fn check_swap_slippage(
    input_amount: u64,
    input_decimals: u8,
    output_amount: u64,
    output_decimals: u8,
    max_slippage_bps: u16,
    oracle_price: i128,
    precision: u32,
) -> ProgramResult {
    // The External address repaid ALL of their tokens, thus we can skip
    // the slippage check as any amount of output tokens is ok.
    if input_amount == 0 {
        return Ok(());
    } else if output_amount == 0 {
        // Error with insufficient funds as we're using the wallets
        // change in balance
        return Err(ProgramError::InsufficientFunds);
    }

    let swap_price = calc_swap_price(
        pow10(input_decimals.into()).unwrap(),
        pow10(output_decimals.into()).unwrap(),
        pow10(precision).unwrap(),
        output_amount.into(),
        input_amount.into(),
    )?;

    // min_swap_price = oracle.value * (100-max_slippage)%
    let min_swap_price = oracle_price
        .checked_mul(BPS_DENOMINATOR.saturating_sub(max_slippage_bps).into())
        .unwrap()
        .checked_ceil_div(BPS_DENOMINATOR.into())
        .unwrap();

    if swap_price < min_swap_price {
        return Err(SvmAlmControllerErrors::SlippageExceeded.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_base_asset_slippage_pass() {
        // Swap Price: $200, Min Oracle Price = $190.935
        let res = check_swap_slippage(
            2_000_000, // input: 2 base token
            6,
            400_000_000, // output: $400
            6,
            1000, // 10%
            202_150_000,
            6,
        );
        assert!(res.is_ok());

        // 0 input with any output is ok
        let res = check_swap_slippage(
            0,
            6,
            400_000_000, // output: $400
            6,
            100, // 1%
            202_150_000,
            6,
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_swap_base_asset_slippage_fail() {
        // Swap Price: $200, Min Oracle Price = $200.1285
        let res = check_swap_slippage(
            2_000_000, // input: 2 base token
            6,
            400_000_000, // output: $400
            6,
            100, // 1%
            202_150_000,
            6,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_swap_zero_output() {
        let res = check_swap_slippage(
            2_000_000,
            6,
            0,
            6,
            100, // 1%
            202_150_000,
            6,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_calc_swap_price() {
        let in_factor = 1_000_000; // 1e6
        let out_factor = 1_000_000_000_000; // 1e12
        let prec_factor = 1_000_000_000_000_000_000; // 1e18
        let input_amount = 2_000_000_000_000; // 2e12
        let output_amount = 4_000_000_000_000; // 4e12
        let price = calc_swap_price(
            in_factor,
            out_factor,
            prec_factor,
            output_amount,
            input_amount,
        )
        .unwrap();

        // Numerator does not exceed i128.
        // (4e12 * 1e6 * 1e18) / (2e12 * 1e12) = 2e12
        assert_eq!(price, 2_000_000_000_000);
    }

    #[test]
    fn test_calc_swap_price_large() {
        let in_factor = 1_000_000_000_000; // 1e12
        let out_factor = 1_000_000_000_000; // 1e12
        let prec_factor = 1_000_000_000_000_000_000; // 1e18
        let input_amount = 2_000_000_000_000; // 2e12
        let output_amount = 4_000_000_000_000; // 4e12
        let price = calc_swap_price(
            in_factor,
            out_factor,
            prec_factor,
            output_amount,
            input_amount,
        )
        .unwrap();

        // Numerator exceeds i128, but handled by division order.
        // (4e12 * 1e12 * 1e18) / (2e12 * 1e12) = 2e18
        assert_eq!(price, 2_000_000_000_000_000_000);
    }
}
