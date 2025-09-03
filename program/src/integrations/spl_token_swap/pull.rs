use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    instructions::PullArgs,
    integrations::spl_token_swap::{
        cpi::withdraw_single_token_type_exact_amount_out_cpi,
        shared_sync::{calculate_prorated_balance, sync_spl_token_swap_integration},
        swap_state::{SwapV1Subset, LEN_SWAP_V1_SUBSET},
    },
    processor::PullAccounts,
    state::{Controller, Integration, Permission, Reserve},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_token_interface::{Mint, TokenAccount};

define_account_struct! {
    pub struct PullSplTokenSwapAccounts<'info> {
        swap: mut;
        mint_a;
        mint_b;
        lp_mint: mut;
        lp_token_account: mut, @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        mint_a_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        mint_b_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        lp_mint_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        swap_token_a: mut;
        swap_token_b: mut;
        vault_a: mut;
        vault_b: mut;
        swap_program;
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
        swap_authority;
        swap_fee_account;
    }
}

impl<'info> PullSplTokenSwapAccounts<'info> {
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
        if !ctx.swap.is_owned_by(ctx.swap_program.key()) {
            msg! {"pool: not owned by swap_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.swap.is_owned_by(&config.program) {
            msg! {"swap: not owned by swap_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.swap_program.key().ne(&config.program) {
            msg! {"swap_program: does not match config"};
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
        if config.mint_a.ne(ctx.mint_a.key()) {
            msg! {"mint_a: does not match IntegrationConfig"};
            return Err(ProgramError::InvalidAccountData);
        }
        if config.mint_b.ne(ctx.mint_b.key()) {
            msg! {"mint_b: does not match IntegrationConfig"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.mint_a.is_owned_by(ctx.mint_a_token_program.key()) {
            msg! {"mint_a: not owned by mint_a_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.mint_b.is_owned_by(ctx.mint_b_token_program.key()) {
            msg! {"mint_b: not owned by mint_b_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.lp_mint.is_owned_by(ctx.lp_mint_token_program.key()) {
            msg! {"lp_mint: not owned by lp_mint_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.swap_token_a.is_owned_by(ctx.mint_a_token_program.key()) {
            msg! {"swap_token_a: not owned by mint_a_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.swap_token_b.is_owned_by(ctx.mint_b_token_program.key()) {
            msg! {"swap_token_b: not owned by mint_b_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        let swap_token_a = TokenAccount::from_account_info(ctx.swap_token_a)?;
        if swap_token_a.mint().ne(&config.mint_a) {
            msg! {"swap_token_a: invalid mint"};
            return Err(ProgramError::InvalidAccountData);
        }
        let swap_token_b = TokenAccount::from_account_info(ctx.swap_token_b)?;
        if swap_token_b.mint().ne(&config.mint_b) {
            msg! {"swap_token_b: invalid mint"};
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

pub fn process_pull_spl_token_swap(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve_a: &mut Reserve,
    reserve_b: &mut Reserve,
    outer_ctx: &PullAccounts,
    outer_args: &PullArgs,
) -> Result<(), ProgramError> {
    msg!("process_pull_spl_token_swap");

    // Get the current slot and time
    let clock = Clock::get()?;

    let (amount_a, amount_b, maximum_pool_token_amount) = match outer_args {
        PullArgs::SplTokenSwap {
            amount_a,
            amount_b,
            maximum_pool_token_amount,
        } => (*amount_a, *amount_b, *maximum_pool_token_amount),
        _ => return Err(ProgramError::InvalidAccountData),
    };
    if amount_a == 0 && amount_b == 0 {
        msg! {"amount_a or amount_b must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    // Check permission
    if !permission.can_reallocate() {
        msg! {"permission: can_reallocate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PullSplTokenSwapAccounts::checked_from_accounts(
        outer_ctx.controller_authority.key(),
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    // Check against reserve data
    if inner_ctx.vault_a.key().ne(&reserve_a.vault) {
        msg! {"vault_a: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }
    if inner_ctx.vault_b.key().ne(&reserve_b.vault) {
        msg! {"vault_b: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }
    if inner_ctx.mint_a.key().ne(&reserve_a.mint) {
        msg! {"mint_a: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }
    if inner_ctx.mint_b.key().ne(&reserve_b.mint) {
        msg! {"mint_b: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }

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

    // Perform a SYNC on Reserve A
    reserve_a.sync_balance(
        inner_ctx.vault_a,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // Perform a SYNC on Reserve B
    reserve_b.sync_balance(
        inner_ctx.vault_b,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // Perform Sync to calcualte the updated balances and emit pre-pull accounting events.
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
        inner_ctx.mint_a.key(),
        inner_ctx.mint_b.key(),
    )?;

    // Track vault balances before withdraw for balance change.
    let (vault_balance_a_before, vault_balance_b_before) = {
        (
            TokenAccount::from_account_info(inner_ctx.vault_a)?.amount(),
            TokenAccount::from_account_info(inner_ctx.vault_b)?.amount(),
        )
    };

    // Carry out the actual withdraw logic
    //  CPI'ing into the SPL Token Swap program
    if amount_a > 0 {
        withdraw_single_token_type_exact_amount_out_cpi(
            amount_a,
            Signer::from(&[
                Seed::from(CONTROLLER_AUTHORITY_SEED),
                Seed::from(outer_ctx.controller.key()),
                Seed::from(&[controller.authority_bump]),
            ]),
            *inner_ctx.swap_program.key(),
            inner_ctx.swap,
            inner_ctx.swap_authority,
            outer_ctx.controller_authority,
            inner_ctx.vault_a,
            inner_ctx.swap_token_a,
            inner_ctx.swap_token_b,
            inner_ctx.lp_mint,
            inner_ctx.lp_token_account,
            inner_ctx.mint_a,
            inner_ctx.mint_a_token_program,
            inner_ctx.lp_mint_token_program,
            inner_ctx.swap_fee_account,
            maximum_pool_token_amount,
        )?;
    }
    if amount_b > 0 {
        withdraw_single_token_type_exact_amount_out_cpi(
            amount_b,
            Signer::from(&[
                Seed::from(CONTROLLER_AUTHORITY_SEED),
                Seed::from(outer_ctx.controller.key()),
                Seed::from(&[controller.authority_bump]),
            ]),
            *inner_ctx.swap_program.key(),
            inner_ctx.swap,
            inner_ctx.swap_authority,
            outer_ctx.controller_authority,
            inner_ctx.vault_b,
            inner_ctx.swap_token_a,
            inner_ctx.swap_token_b,
            inner_ctx.lp_mint,
            inner_ctx.lp_token_account,
            inner_ctx.mint_b,
            inner_ctx.mint_b_token_program,
            inner_ctx.lp_mint_token_program,
            inner_ctx.swap_fee_account,
            maximum_pool_token_amount,
        )?;
    }

    // Calculate the change in vault balances.
    // We must use the amounts in the TokenAccounts to ensure
    // proper accounting when TransferFee is enabled OR
    // withdrawal is not exact amount.
    let (vault_balance_a_after, vault_balance_b_after) = {
        (
            TokenAccount::from_account_info(inner_ctx.vault_a)?.amount(),
            TokenAccount::from_account_info(inner_ctx.vault_b)?.amount(),
        )
    };
    let vault_balance_a_delta = vault_balance_a_after
        .checked_sub(vault_balance_a_before)
        .unwrap();
    let vault_balance_b_delta = vault_balance_b_after
        .checked_sub(vault_balance_b_before)
        .unwrap();

    // Refresh values for LP Mint supply, LP tokens held
    //  and swap pool owned balances for tokens a and b
    let lp_token_account = TokenAccount::from_account_info(inner_ctx.lp_token_account)?;
    let post_withdraw_balance_lp = lp_token_account.amount();
    let lp_mint = Mint::from_account_info(inner_ctx.lp_mint)?;
    let lp_mint_supply = lp_mint.supply();

    let swap_token_a = TokenAccount::from_account_info(inner_ctx.swap_token_a)?;
    let swap_token_b = TokenAccount::from_account_info(inner_ctx.swap_token_b)?;
    let delta_lp = latest_balance_lp
        .checked_sub(post_withdraw_balance_lp)
        .unwrap();

    // Determine the share of the pool's a and b tokens that we have a claim on
    let post_withdraw_balance_a: u64;
    let post_withdraw_balance_b: u64;
    if post_withdraw_balance_lp > 0 {
        post_withdraw_balance_a = calculate_prorated_balance(
            swap_token_a.amount(),
            post_withdraw_balance_lp,
            lp_mint_supply,
        );
        post_withdraw_balance_b = calculate_prorated_balance(
            swap_token_b.amount(),
            post_withdraw_balance_lp,
            lp_mint_supply,
        );
    } else {
        post_withdraw_balance_a = 0u64;
        post_withdraw_balance_b = 0u64;
    }

    // Update the state for the Pre-Push changes
    match &mut integration.state {
        IntegrationState::SplTokenSwap(state) => {
            state.last_balance_a = post_withdraw_balance_a;
            state.last_balance_b = post_withdraw_balance_b;
            state.last_balance_lp = post_withdraw_balance_lp as u64;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    // Update the integration rate limit for the outflow
    //  Rate limit for the SplTokenSwap is (counterintuitively) tracked in
    //  units of LP tokens (out, for tokens a or b in)
    integration.update_rate_limit_for_inflow(clock, delta_lp as u64)?;

    // Update the reserves for the flows
    if vault_balance_a_delta > 0 {
        reserve_a.update_for_inflow(clock, vault_balance_a_delta)?;
    }
    if vault_balance_b_delta > 0 {
        reserve_b.update_for_inflow(clock, vault_balance_b_delta)?;
    }

    // Emit the accounting event
    if latest_balance_a != post_withdraw_balance_a {
        controller.emit_event(
            outer_ctx.controller_authority,
            outer_ctx.controller.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: *inner_ctx.mint_a.key(),
                action: AccountingAction::Withdrawal,
                before: latest_balance_a,
                after: post_withdraw_balance_a,
            }),
        )?;
    }
    // Emit the accounting event
    if latest_balance_b != post_withdraw_balance_b {
        controller.emit_event(
            outer_ctx.controller_authority,
            outer_ctx.controller.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: *inner_ctx.mint_b.key(),
                action: AccountingAction::Withdrawal,
                before: latest_balance_b,
                after: post_withdraw_balance_b,
            }),
        )?;
    }

    Ok(())
}
