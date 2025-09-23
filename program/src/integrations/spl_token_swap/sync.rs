use super::swap_state::{SwapV1Subset, LEN_SWAP_V1_SUBSET};
use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    integrations::spl_token_swap::shared_sync::sync_spl_token_swap_integration,
    processor::SyncIntegrationAccounts,
    state::{Controller, Integration},
};
use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_token_interface::TokenAccount;

define_account_struct! {
    pub struct SyncSplTokenSwapAccounts<'info> {
        swap;
        lp_mint;
        lp_token_account;
        swap_token_a;
        swap_token_b;
    }
}

impl<'info> SyncSplTokenSwapAccounts<'info> {
    pub fn checked_from_accounts(
        controller_authority: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::SplTokenSwap(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if !ctx.swap.is_owned_by(&config.program) {
            msg! {"swap: not owned by swap_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.swap.key().ne(&config.swap) {
            msg! {"swap: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.lp_mint.key().ne(&config.lp_mint) {
            msg! {"lp_mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.lp_token_account.key().ne(&config.lp_token_account) {
            msg! {"lp_token_account: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        let lp_token_account = TokenAccount::from_account_info(ctx.lp_token_account)?;
        if lp_token_account.mint().ne(&config.lp_mint) {
            msg! {"lp_token_account: invalid mint"};
            return Err(ProgramError::InvalidAccountData);
        }
        if lp_token_account.owner().ne(controller_authority) {
            msg! {"lp_token_account: not owned by Controller authority PDA"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

pub fn process_sync_spl_token_swap(
    controller: &Controller,
    integration: &mut Integration,
    outer_ctx: &SyncIntegrationAccounts,
) -> Result<(), ProgramError> {
    let inner_ctx = SyncSplTokenSwapAccounts::checked_from_accounts(
        outer_ctx.controller_authority.key(),
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    // Load in the Pool state and verify the accounts
    //  w.r.t it's stored state
    let swap_data = inner_ctx.swap.try_borrow_data()?;
    let swap_state = SwapV1Subset::try_from_slice(&swap_data[1..LEN_SWAP_V1_SUBSET + 1])
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    if swap_state.pool_mint.ne(inner_ctx.lp_mint.key()) {
        msg! {"lp_mint: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }
    if swap_state.token_a.ne(inner_ctx.swap_token_a.key()) {
        msg! {"swap_token_a: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }
    if swap_state.token_b.ne(inner_ctx.swap_token_b.key()) {
        msg! {"swap_token_b: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Extract the values from the config
    let (mint_a_key, mint_b_key) = match integration.config {
        IntegrationConfig::SplTokenSwap(config) => (config.mint_a, config.mint_b),
        _ => return Err(ProgramError::InvalidAccountData),
    };

    // Calculate the updated balances and emit accounting events.
    let (latest_balance_a, latest_balance_b, latest_balance_lp) = sync_spl_token_swap_integration(
        controller,
        integration,
        outer_ctx.controller,
        outer_ctx.controller_authority,
        outer_ctx.integration,
        inner_ctx.swap_token_a,
        inner_ctx.swap_token_b,
        inner_ctx.lp_token_account,
        inner_ctx.lp_mint,
        &mint_a_key,
        &mint_b_key,
    )?;

    // Update the state
    match &mut integration.state {
        IntegrationState::SplTokenSwap(state) => {
            // Prevent spamming/ddos attacks -- since the sync ixn is permissionless
            //  calling this repeatedly could bombard the program and indevers
            if state.last_balance_a == latest_balance_a
                && state.last_balance_b == latest_balance_b
                && state.last_balance_lp == latest_balance_lp
            {
                return Err(SvmAlmControllerErrors::DataNotChangedSinceLastSync.into());
            }
            state.last_balance_a = latest_balance_a;
            state.last_balance_b = latest_balance_b;
            state.last_balance_lp = latest_balance_lp;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}
