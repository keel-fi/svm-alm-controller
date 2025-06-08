use crate::{
    enums::{IntegrationConfig, IntegrationState},
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::atomic_swap::{config::AtomicSwapConfig, state::AtomicSwapState},
    processor::InitializeIntegrationAccounts,
    wrapper::{MintAccount, OracleAccount, WrappedAccount},
};
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};

pub struct InitializeAtomicSwapAccounts<'info> {
    pub input_mint: MintAccount<'info>,
    pub output_mint: MintAccount<'info>,
    pub oracle: OracleAccount<'info>,
}

impl<'info> InitializeAtomicSwapAccounts<'info> {
    pub fn from_accounts(account_infos: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if account_infos.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        let ctx = Self {
            input_mint: MintAccount::new(&account_infos[0])?,
            output_mint: MintAccount::new(&account_infos[1])?,
            oracle: OracleAccount::new(&account_infos[2])?,
        };
        Ok(ctx)
    }
}

pub fn process_initialize_atomic_swap(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_atomic_swap");

    let inner_ctx = InitializeAtomicSwapAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    let InitializeArgs::AtomicSwap {
        max_slippage_bps,
        max_staleness,
        expiry_timestamp,
        ..
    } = outer_args.inner_args
    else {
        return Err(ProgramError::InvalidArgument);
    };

    let clock = Clock::get()?;
    if max_staleness >= clock.slot || expiry_timestamp <= clock.unix_timestamp {
        return Err(ProgramError::InvalidArgument);
    }

    // Create the Config
    let config = IntegrationConfig::AtomicSwap(AtomicSwapConfig {
        input_token: *inner_ctx.input_mint.key(),
        output_token: *inner_ctx.output_mint.key(),
        oracle: *inner_ctx.oracle.key(),
        max_slippage_bps,
        max_staleness,
        input_mint_decimals: inner_ctx.input_mint.inner().decimals(),
        output_mint_decimals: inner_ctx.output_mint.inner().decimals(),
        expiry_timestamp,
        padding: [0u8; 76],
    });

    // Create the initial integration state
    let state = IntegrationState::AtomicSwap(AtomicSwapState {
        last_balance_a: 0,
        last_balance_b: 0,
        amount_borrowed: 0,
        recipient_token_a_pre: 0,
        repay_excess_token_a: false,
        _padding: [0u8; 15],
    });

    Ok((config, state))
}
