use borsh::BorshDeserialize;
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
    instructions::AtomicSwapRepayArgs,
    state::{nova_account::NovaAccount, Integration, Oracle, Permission, Reserve},
};

define_account_struct! {
    pub struct AtomicSwapRepay<'info> {
        payer: signer;
        controller;
        authority: signer;
        permission;
        integration: mut;
        reserve_a: mut;
        vault_a: mut;
        mint_a;
        reserve_b: mut;
        vault_b: mut;
        mint_b;
        oracle;
        payer_account_a: mut;
        payer_account_b: mut;
        token_program_a: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program_b: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
    }
}

pub fn process_atomic_swap_repay(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("atomic_swap_repay");
    let ctx = AtomicSwapRepay::from_accounts(accounts)?;
    let args: AtomicSwapRepayArgs = AtomicSwapRepayArgs::try_from_slice(instruction_data).unwrap();
    let clock = Clock::get()?;

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    if !permission.can_execute_swap() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Check that mint and vault account matches known keys in controller-associated Reserve.
    let mut reserve_a = Reserve::load_and_check_mut(ctx.reserve_a, ctx.controller.key())?;
    if reserve_a.vault != *ctx.vault_a.key() || reserve_a.mint.ne(ctx.mint_a.key()) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    let mut reserve_b = Reserve::load_and_check_mut(ctx.reserve_b, ctx.controller.key())?;
    if reserve_b.vault != *ctx.vault_b.key() || reserve_b.mint.ne(ctx.mint_b.key()) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }

    // Check that Integration account is valid and matches controller.
    let mut integration = Integration::load_and_check_mut(ctx.integration, ctx.controller.key())?;
    let mut excess_token_a = 0;

    if let (IntegrationConfig::AtomicSwap(cfg), IntegrationState::AtomicSwap(state)) =
        (&integration.config, &mut integration.state)
    {
        if cfg.input_token != reserve_a.mint
            || cfg.output_token != reserve_b.mint
            || cfg.oracle != *ctx.oracle.key()
        {
            return Err(SvmAlmControllerErrors::InvalidAccountData.into());
        }

        if !state.has_swap_started() {
            return Err(SvmAlmControllerErrors::SwapNotStarted.into());
        }

        let mut final_input_amount = state.amount_borrowed;
        {
            // Check that vault_a and vault_b amounts remain same as after atomic borrow.
            let vault_a = TokenAccount::from_account_info(ctx.vault_a)?;
            let vault_b = TokenAccount::from_account_info(ctx.vault_b)?;
            let payer_account_b = TokenAccount::from_account_info(ctx.payer_account_b)?;

            // Check that vault_a and vault_b balances are not modified between atomic borrow and repay.
            if vault_a.amount().checked_add(state.amount_borrowed).unwrap() != state.last_balance_a
                || vault_b.amount() != state.last_balance_b
            {
                return Err(SvmAlmControllerErrors::InvalidSwapState.into());
            }

            if state.repay_excess_token_a {
                let payer_account_a = TokenAccount::from_account_info(ctx.payer_account_a)?;
                excess_token_a = payer_account_a
                    .amount()
                    .saturating_sub(state.recipient_token_a_pre);
                final_input_amount = final_input_amount.checked_sub(excess_token_a).unwrap();
            }

            if args.amount > payer_account_b.amount() {
                return Err(ProgramError::InsufficientFunds);
            }
        }

        // Transfer tokens to vault for repayment.
        if excess_token_a > 0 {
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
        }

        let mint_b = Mint::from_account_info(ctx.mint_b)?;
        TransferChecked {
            from: ctx.payer_account_b,
            to: ctx.vault_b,
            mint: ctx.mint_b,
            authority: ctx.payer,
            amount: args.amount,
            decimals: mint_b.decimals(),
            token_program: ctx.token_program_b.key(),
        }
        .invoke()?;

        let oracle = Oracle::load_and_check(ctx.oracle)?;

        // Check that oracle was last refreshed within acceptable staleness.
        if oracle.last_update_slot < clock.slot - cfg.max_staleness {
            return Err(SvmAlmControllerErrors::StaleOraclePrice.into());
        }

        // TODO does this need to not use args.amount in case of TransferFees?

        // Check that swap is within accepted slippage of oracle price.
        check_swap_slippage(
            final_input_amount,
            cfg.input_mint_decimals,
            args.amount,
            cfg.output_mint_decimals,
            cfg.max_slippage_bps,
            oracle.value,
            oracle.precision,
        )?;

        // Reset state after repayment.
        state.reset();
    } else {
        return Err(SvmAlmControllerErrors::Invalid.into());
    }

    // Update for rate limits and save.
    reserve_a.update_for_inflow(clock, excess_token_a)?;
    reserve_a.save(ctx.reserve_a)?;
    reserve_b.update_for_inflow(clock, args.amount)?;
    reserve_b.save(ctx.reserve_b)?;

    integration.update_rate_limit_for_inflow(clock, excess_token_a)?;
    integration.save(ctx.integration)?;

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
    if input_amount == 0 || output_amount == 0 {
        return Err(ProgramError::InvalidArgument);
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
        .checked_div(BPS_DENOMINATOR.into())
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
    fn test_swap_zero_input_or_output() {
        let res = check_swap_slippage(
            0,
            6,
            400_000_000, // output: $400
            6,
            100, // 1%
            202_150_000,
            6,
        );
        assert!(res.is_err());

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
