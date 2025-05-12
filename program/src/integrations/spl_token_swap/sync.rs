use super::swap_state::{SwapV1Subset, LEN_SWAP_V1_SUBSET};
use crate::{
    enums::{IntegrationConfig, IntegrationState},
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    processor::SyncIntegrationAccounts,
    state::{Controller, Integration},
};
use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_token::{
    self,
    state::{Mint, TokenAccount},
};

pub struct SyncSplTokenSwapAccounts<'info> {
    pub swap: &'info AccountInfo,
    pub lp_mint: &'info AccountInfo,
    pub lp_token_account: &'info AccountInfo,
    pub swap_token_a: &'info AccountInfo,
    pub swap_token_b: &'info AccountInfo,
}

impl<'info> SyncSplTokenSwapAccounts<'info> {
    pub fn checked_from_accounts(
        controller: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            swap: &account_infos[0],
            lp_mint: &account_infos[1],
            lp_token_account: &account_infos[2],
            swap_token_a: &account_infos[3],
            swap_token_b: &account_infos[4],
        };
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
        if lp_token_account.owner().ne(controller) {
            msg! {"lp_token_account: not owned by controller"};
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
        outer_ctx.controller.key(),
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    let lp_mint = Mint::from_account_info(inner_ctx.lp_mint).unwrap();
    let lp_mint_supply = lp_mint.supply() as u128;

    // Load in the Pool state and verify the accounts
    //  w.r.t it's stored state
    let swap_data = inner_ctx.swap.try_borrow_data()?;
    let swap_state = SwapV1Subset::try_from_slice(&swap_data[1..LEN_SWAP_V1_SUBSET + 1]).unwrap();

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

    // Extract the values from the last update
    let (last_balance_a, last_balance_b, last_balance_lp) = match integration.state {
        IntegrationState::SplTokenSwap(state) => (
            state.last_balance_a,
            state.last_balance_b,
            state.last_balance_lp as u128,
        ),
        _ => return Err(ProgramError::InvalidAccountData),
    };

    // STEP 1: Get the changes due to relative movement between token A and B
    // LP tokens constant, relative balance of A and B changed
    // (based on the old number of lp tokens)

    let swap_token_a = TokenAccount::from_account_info(inner_ctx.swap_token_a)?;
    let swap_token_b = TokenAccount::from_account_info(inner_ctx.swap_token_b)?;

    let step_1_balance_a: u64;
    let step_1_balance_b: u64;
    if last_balance_lp > 0 {
        step_1_balance_a =
            (swap_token_a.amount() as u128 * last_balance_lp / lp_mint_supply) as u64;
        step_1_balance_b =
            (swap_token_b.amount() as u128 * last_balance_lp / lp_mint_supply) as u64;
    } else {
        step_1_balance_a = 0u64;
        step_1_balance_b = 0u64;
    }
    // Emit the accounting events for the change in A and B's relative balances
    if last_balance_a != step_1_balance_a {
        controller.emit_event(
            outer_ctx.controller,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: mint_a_key,
                action: AccountingAction::Sync,
                before: last_balance_a,
                after: step_1_balance_a,
            }),
        )?;
    }
    if last_balance_b != step_1_balance_b {
        controller.emit_event(
            outer_ctx.controller,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: mint_b_key,
                action: AccountingAction::Sync,
                before: last_balance_b,
                after: step_1_balance_b,
            }),
        )?;
    }

    // Load in the vault, since it could have an opening balance
    let lp_token_account = TokenAccount::from_account_info(inner_ctx.lp_token_account)?;
    let new_balance_lp = lp_token_account.amount() as u128;

    // STEP 2: If the number of LP tokens changed
    // We need to account for the change in our claim
    //  on the underlying A and B tokens as a result of this
    //  change in LP tokens

    let step_2_balance_a: u64;
    let step_2_balance_b: u64;
    if new_balance_lp != last_balance_lp {
        if new_balance_lp > 0 {
            step_2_balance_a =
                (swap_token_a.amount() as u128 * new_balance_lp / lp_mint_supply) as u64;
            step_2_balance_b =
                (swap_token_b.amount() as u128 * new_balance_lp / lp_mint_supply) as u64;
        } else {
            step_2_balance_a = 0u64;
            step_2_balance_b = 0u64;
        }
        // Emit the accounting events for the change in A and B's relative balances
        controller.emit_event(
            outer_ctx.controller,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: mint_a_key,
                action: AccountingAction::Sync,
                before: step_1_balance_a,
                after: step_2_balance_a,
            }),
        )?;
        controller.emit_event(
            outer_ctx.controller,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: mint_b_key,
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

    // Update the state
    match &mut integration.state {
        IntegrationState::SplTokenSwap(state) => {
            // Prevent spamming/ddos attacks -- since the sync ixn is permissionless
            //  calling this repeatedly could bombard the program and indevers
            if state.last_balance_a == step_2_balance_a
                && state.last_balance_b == step_2_balance_b
                && state.last_balance_lp == new_balance_lp as u64
            {
                return Err(ProgramError::InvalidInstructionData.into());
            }
            state.last_balance_a = step_2_balance_a;
            state.last_balance_b = step_2_balance_b;
            state.last_balance_lp = new_balance_lp as u64;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}
