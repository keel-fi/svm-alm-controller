use crate::{
    enums::{IntegrationConfig, IntegrationState},
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::swap::{config::AtomicSwapConfig, state::AtomicSwapState},
    processor::InitializeIntegrationAccounts,
    state::{nova_account::NovaAccount, Oracle},
};
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError};

pub struct InitializeAtomicSwapAccounts<'info> {
    pub input_mint: &'info AccountInfo,
    pub output_mint: &'info AccountInfo,
    pub oracle: &'info AccountInfo,
}

impl<'info> InitializeAtomicSwapAccounts<'info> {
    // TODO: from_accounts could be a requirement for an Integration trait
    pub fn from_accounts(account_infos: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        // TODO: ACCOUNT_LEN could be a requirement for an Integration trait
        if account_infos.len() != 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            input_mint: &account_infos[0],
            output_mint: &account_infos[1],
            oracle: &account_infos[2],
        };
        if !ctx.input_mint.is_owned_by(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"mint: not owned by token program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // TODO: Valdiate mint structure?
        if !ctx.output_mint.is_owned_by(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"mint: not owned by token program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.oracle.is_owned_by(&crate::ID) {
            msg! {"oracle: not owned by program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // Check that Oracle is a valid account.
        let _oracle: Oracle = NovaAccount::deserialize(&ctx.oracle.try_borrow_data()?)?;

        Ok(ctx)
    }
}

pub fn process_initialize_atomic_swap(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_atomic_swap");

    let inner_ctx = InitializeAtomicSwapAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    let (max_slippage_bps, is_input_token_base_asset, max_staleness) = match outer_args.inner_args {
        InitializeArgs::AtomicSwap {
            max_slippage_bps,
            is_input_token_base_asset,
            max_staleness,
            ..
        } => (max_slippage_bps, is_input_token_base_asset, max_staleness),
        _ => return Err(ProgramError::InvalidArgument),
    };

    // TODO: Add an order expiry date
    // Create the Config
    let config = IntegrationConfig::AtomicSwap(AtomicSwapConfig {
        input_token: *inner_ctx.input_mint.key(),
        output_token: *inner_ctx.output_mint.key(),
        oracle: *inner_ctx.oracle.key(),
        max_slippage_bps,
        is_input_token_base_asset,
        max_staleness,
        padding: [0u8; 85],
    });

    // Create the initial integration state
    let state = IntegrationState::AtomicSwap(AtomicSwapState {
        last_balance_a: 0,
        last_balance_b: 0,
        amount_borrowed: 0,
        _padding: [0u8; 24],
    });

    Ok((config, state))
}
