use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::atomic_swap::{config::AtomicSwapConfig, state::AtomicSwapState},
    processor::InitializeIntegrationAccounts,
    state::Oracle,
};
use pinocchio::{
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_token_interface::Mint;

define_account_struct! {
    pub struct InitializeAtomicSwapAccounts<'info> {
        input_mint;
        output_mint;
        oracle: @owner(crate::ID);
    }
}

pub fn process_initialize_atomic_swap(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_atomic_swap");

    let inner_ctx = InitializeAtomicSwapAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    // Validate no same token swaps
    if inner_ctx.input_mint.key().eq(inner_ctx.output_mint.key()) {
        return Err(SvmAlmControllerErrors::InvalidAtomicSwapConfiguration.into());
    }

    // Check that Oracle is a valid account.
    let _oracle: Oracle = Oracle::load_and_check(&inner_ctx.oracle)?;

    let InitializeArgs::AtomicSwap {
        max_slippage_bps,
        max_staleness,
        expiry_timestamp,
        oracle_price_inverted,
        ..
    } = outer_args.inner_args
    else {
        return Err(ProgramError::InvalidArgument);
    };

    let clock = Clock::get()?;
    if max_staleness >= clock.slot || expiry_timestamp <= clock.unix_timestamp {
        return Err(SvmAlmControllerErrors::StaleOraclePrice.into());
    }

    let input_mint = Mint::from_account_info(inner_ctx.input_mint)?;
    let output_mint = Mint::from_account_info(inner_ctx.output_mint)?;

    // Create the Config
    let config = IntegrationConfig::AtomicSwap(AtomicSwapConfig {
        input_token: *inner_ctx.input_mint.key(),
        output_token: *inner_ctx.output_mint.key(),
        oracle: *inner_ctx.oracle.key(),
        max_slippage_bps,
        max_staleness,
        input_mint_decimals: input_mint.decimals(),
        output_mint_decimals: output_mint.decimals(),
        expiry_timestamp,
        oracle_price_inverted,
        padding: [0u8; 107],
    });

    // Create the initial integration state
    let state = IntegrationState::AtomicSwap(AtomicSwapState {
        last_balance_a: 0,
        last_balance_b: 0,
        amount_borrowed: 0,
        recipient_token_a_pre: 0,
        recipient_token_b_pre: 0,
        _padding: [0u8; 8],
    });

    Ok((config, state))
}
