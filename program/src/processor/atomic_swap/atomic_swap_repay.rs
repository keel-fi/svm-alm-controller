use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token::{instructions::Transfer, state::TokenAccount};

use crate::{
    constants::BPS_DENOMINATOR,
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    instructions::AtomicSwapRepayArgs,
    state::{Integration, Oracle, Permission, Reserve},
};

pub struct AtomicSwapRepay<'info> {
    pub payer: &'info AccountInfo,
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub reserve_a: &'info AccountInfo,
    pub vault_a: &'info AccountInfo,
    pub reserve_b: &'info AccountInfo,
    pub vault_b: &'info AccountInfo,
    pub oracle: &'info AccountInfo,
    pub payer_token_account: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
}

impl<'info> AtomicSwapRepay<'info> {
    // TODO: Let Reserve be mutable to enforce rate limits?
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 12 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            payer: &accounts[0],
            controller: &accounts[1],
            authority: &accounts[2],
            permission: &accounts[3],
            integration: &accounts[4],
            reserve_a: &accounts[5],
            vault_a: &accounts[6],
            reserve_b: &accounts[7],
            vault_b: &accounts[8],
            oracle: &accounts[9],
            payer_token_account: &accounts[10],
            token_program: &accounts[11],
        };
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.integration.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !ctx.vault_b.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !ctx.payer_token_account.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if ctx.token_program.key().ne(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(ctx)
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

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    // TODO: Verify that this is the right permission to check
    if !permission.can_execute_swap() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Check that mint and vault account matches known keys in controller-associated Reserve.
    let reserve_a = Reserve::load_and_check(ctx.reserve_a, ctx.controller.key())?;
    if reserve_a.vault != *ctx.vault_a.key() {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    let reserve_b = Reserve::load_and_check(ctx.reserve_b, ctx.controller.key())?;
    if reserve_b.vault != *ctx.vault_b.key() {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }

    // Check that Integration account is valid and matches controller.
    let integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;

    if let (IntegrationConfig::AtomicSwap(cfg), IntegrationState::AtomicSwap(mut state)) =
        (&integration.config, integration.state)
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

        {
            // Check that vault_a and vault_b amounts remain same as after atomic borrow.
            let vault_a = TokenAccount::from_account_info(ctx.vault_a)?;
            let vault_b = TokenAccount::from_account_info(ctx.vault_b)?;
            let payer_token_account = TokenAccount::from_account_info(ctx.payer_token_account)?;

            if vault_a.amount().checked_add(state.amount_borrowed).unwrap() != state.last_balance_a
                || vault_b.amount() != state.last_balance_b
            {
                return Err(SvmAlmControllerErrors::InvalidSwapState.into());
            }

            if args.amount > payer_token_account.amount() {
                return Err(ProgramError::InsufficientFunds);
            }
        }

        // Transfer amount from payer_token_account to vault_b.
        Transfer {
            from: ctx.payer_token_account,
            to: ctx.vault_b,
            authority: ctx.payer,
            amount: args.amount,
        }
        .invoke()?;

        let oracle = Oracle::load_and_check(ctx.oracle)?;

        // Check that oracle was last refreshed within acceptable staleness.
        let clock = Clock::get()?;
        if oracle.last_update_slot < clock.slot - cfg.max_staleness {
            return Err(SvmAlmControllerErrors::StaleOraclePrice.into());
        }

        // Check that swap is within accepted slippage of oracle price.
        check_swap_slippage(
            state.amount_borrowed,
            cfg.input_mint_decimals,
            args.amount,
            cfg.output_mint_decimals,
            cfg.max_slippage_bps,
            cfg.is_input_token_base_asset,
            &oracle,
        )?;
    } else {
        return Err(SvmAlmControllerErrors::Invalid.into());
    }

    // Close integration account and transfer rent to payer.
    let payer_lamports = ctx.payer.lamports();
    *ctx.payer.try_borrow_mut_lamports().unwrap() = payer_lamports
        .checked_add(ctx.integration.lamports())
        .unwrap();
    *ctx.integration.try_borrow_mut_lamports().unwrap() = 0;
    ctx.integration.close()?;

    Ok(())
}

fn pow10(decimals: u32) -> Option<i128> {
    10_i128.checked_pow(decimals)
}

pub fn check_swap_slippage(
    input_amount: u64,
    input_decimals: u8,
    output_amount: u64,
    output_decimals: u8,
    max_slippage_bps: u16,
    is_input_token_base_asset: bool,
    oracle: &Oracle,
) -> ProgramResult {
    if input_amount == 0 || output_amount == 0 {
        return Err(ProgramError::InvalidArgument);
    }
    let in_factor = pow10(input_decimals.into()).unwrap();
    let out_factor = pow10(output_decimals.into()).unwrap();
    let prec_factor = pow10(oracle.precision).unwrap();

    if is_input_token_base_asset {
        // swap_price = (output_amount / out_factor) / (input_amount / in_factor)
        let swap_price = i128::from(output_amount)
            .checked_mul(in_factor)
            .unwrap()
            .checked_mul(prec_factor)
            .unwrap()
            .checked_div(input_amount.into())
            .unwrap()
            .checked_div(out_factor)
            .unwrap();

        // min_swap_price = oracle.value * (100-max_slippage)%
        let min_swap_price = oracle
            .value
            .checked_mul(BPS_DENOMINATOR.saturating_sub(max_slippage_bps).into())
            .unwrap()
            .checked_div(BPS_DENOMINATOR.into())
            .unwrap();

        if swap_price < min_swap_price {
            return Err(SvmAlmControllerErrors::SlippageExceeded.into());
        }
        Ok(())
    } else {
        // swap_price = (input_amount / in_factor) / (output_amount / out_factor)
        let swap_price = i128::from(input_amount)
            .checked_mul(out_factor)
            .unwrap()
            .checked_mul(prec_factor)
            .unwrap()
            .checked_div(output_amount.into())
            .unwrap()
            .checked_div(in_factor)
            .unwrap();

        // max_swap_price = oracle.value * (100+max_slippage)%
        let max_swap_price = oracle
            .value
            .checked_mul(
                (BPS_DENOMINATOR as u32)
                    .saturating_add(max_slippage_bps.into())
                    .into(),
            )
            .unwrap()
            .checked_div(BPS_DENOMINATOR.into())
            .unwrap();

        if swap_price > max_swap_price {
            return Err(SvmAlmControllerErrors::SlippageExceeded.into());
        }
        Ok(())
    }
}
