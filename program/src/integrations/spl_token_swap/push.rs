use crate::{
    constants::CONTROLLER_SEED,
    enums::{IntegrationConfig, IntegrationState},
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::spl_token_swap::{
        cpi::deposit_single_token_type_exact_amount_in_cpi,
        swap_state::{SwapV1Subset, LEN_SWAP_V1_SUBSET},
    },
    processor::PushAccounts,
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
use pinocchio_token::{
    self,
    state::{Mint, TokenAccount},
};

pub struct PushSplTokenSwapAccounts<'info> {
    pub swap: &'info AccountInfo,
    pub mint_a: &'info AccountInfo,
    pub mint_b: &'info AccountInfo,
    pub lp_mint: &'info AccountInfo,
    pub lp_token_account: &'info AccountInfo,
    pub mint_a_token_program: &'info AccountInfo,
    pub mint_b_token_program: &'info AccountInfo,
    pub lp_mint_token_program: &'info AccountInfo,
    pub swap_token_a: &'info AccountInfo,
    pub swap_token_b: &'info AccountInfo,
    pub vault_a: &'info AccountInfo,
    pub vault_b: &'info AccountInfo,
    pub swap_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
    pub swap_authority: &'info AccountInfo,
    pub swap_fee_account: &'info AccountInfo,
}

impl<'info> PushSplTokenSwapAccounts<'info> {
    pub fn checked_from_accounts(
        controller: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 16 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            swap: &account_infos[0],
            mint_a: &account_infos[1],
            mint_b: &account_infos[2],
            lp_mint: &account_infos[3],
            lp_token_account: &account_infos[4],
            mint_a_token_program: &account_infos[5],
            mint_b_token_program: &account_infos[6],
            lp_mint_token_program: &account_infos[7],
            swap_token_a: &account_infos[8],
            swap_token_b: &account_infos[9],
            vault_a: &account_infos[10],
            vault_b: &account_infos[11],
            swap_program: &account_infos[12],
            associated_token_program: &account_infos[13],
            swap_authority: &account_infos[14],
            swap_fee_account: &account_infos[15],
        };
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
        if ctx.mint_a_token_program.key().ne(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"mint_a_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.mint_b_token_program.key().ne(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"mint_b_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.lp_mint_token_program.key().ne(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"lp_mint_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx
            .associated_token_program
            .key()
            .ne(&pinocchio_associated_token_account::ID)
        {
            msg! {"associated_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if !ctx.lp_token_account.is_writable() {
            msg! {"lp_token_account: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx
            .lp_token_account
            .is_owned_by(ctx.lp_mint_token_program.key())
            && !ctx.lp_token_account.is_owned_by(&pinocchio_system::ID)
        {
            msg! {"lp_token_account: not owned by token_program or system_program"};
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
        if !ctx.swap.is_writable() {
            msg! {"swap: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.lp_mint.is_writable() {
            msg! {"lp_mint: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.lp_token_account.is_writable() {
            msg! {"lp_mint: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.swap_token_a.is_writable() {
            msg! {"swap_token_a: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.swap_token_b.is_writable() {
            msg! {"swap_token_b: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.vault_a.is_writable() {
            msg! {"vault_a: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.vault_b.is_writable() {
            msg! {"vault_b: not mutable"};
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

pub fn process_push_spl_token_swap(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve_a: &mut Reserve,
    reserve_b: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    msg!("process_push_spl_token_swap");

    // Get the current slot and time
    let clock = Clock::get()?;
    let controller_id_bytes = controller.id.to_le_bytes();
    let controller_bump = controller.bump;

    let (amount_a, amount_b) = match outer_args {
        PushArgs::SplTokenSwap { amount_a, amount_b } => (*amount_a, *amount_b),
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

    let inner_ctx = PushSplTokenSwapAccounts::checked_from_accounts(
        outer_ctx.controller.key(),
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    // Check against reserve data
    if inner_ctx.vault_a.key().ne(&reserve_a.vault) {
        msg! {"mint_a: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }
    if inner_ctx.vault_b.key().ne(&reserve_b.vault) {
        msg! {"vault_b: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountOwner);
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
    let swap_state = SwapV1Subset::try_from_slice(&swap_data[1..LEN_SWAP_V1_SUBSET + 1]).unwrap();
    drop(swap_data);

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

    // // Perform a SYNC on Reserve A
    reserve_a.sync_balance(inner_ctx.vault_a, outer_ctx.controller, controller)?;

    // Perform a SYNC on Reserve B
    reserve_b.sync_balance(inner_ctx.vault_b, outer_ctx.controller, controller)?;

    // // Perform SYNC on LP Tokens

    // // Extract the values from the last update
    let (last_balance_a, last_balance_b, last_balance_lp) = match integration.state {
        IntegrationState::SplTokenSwap(state) => (
            state.last_balance_a,
            state.last_balance_b,
            state.last_balance_lp as u128,
        ),
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let lp_mint = Mint::from_account_info(inner_ctx.lp_mint).unwrap();
    let lp_mint_supply = lp_mint.supply() as u128;
    drop(lp_mint);

    // STEP 1: Get the changes due to relative movement between token A and B
    // LP tokens constant, relative balance of A and B changed
    // (based on the old number of lp tokens)

    let swap_token_a = TokenAccount::from_account_info(inner_ctx.swap_token_a)?;
    let swap_token_b = TokenAccount::from_account_info(inner_ctx.swap_token_b)?;
    let swap_token_a_balance = swap_token_a.amount();
    let swap_token_b_balance = swap_token_b.amount();
    drop(swap_token_a);
    drop(swap_token_b);

    let step_1_balance_a: u64;
    let step_1_balance_b: u64;
    if last_balance_lp > 0 {
        step_1_balance_a = (swap_token_a_balance as u128 * last_balance_lp / lp_mint_supply) as u64;
        step_1_balance_b = (swap_token_b_balance as u128 * last_balance_lp / lp_mint_supply) as u64;
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
                mint: *inner_ctx.mint_a.key(),
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
                mint: *inner_ctx.mint_b.key(),
                action: AccountingAction::Sync,
                before: last_balance_b,
                after: step_1_balance_b,
            }),
        )?;
    }

    // Load in the vault, since it could have an opening balance
    let lp_token_account = TokenAccount::from_account_info(inner_ctx.lp_token_account)?;
    let step_2_balance_lp = lp_token_account.amount() as u128;
    drop(lp_token_account);

    // STEP 2: If the number of LP tokens changed
    // We need to account for the change in our claim
    //  on the underlying A and B tokens as a result of this
    //  change in LP tokens

    let step_2_balance_a: u64;
    let step_2_balance_b: u64;
    if step_2_balance_lp != last_balance_lp {
        if step_2_balance_lp > 0 {
            step_2_balance_a =
                (swap_token_a_balance as u128 * step_2_balance_lp / lp_mint_supply) as u64;
            step_2_balance_b =
                (swap_token_b_balance as u128 * step_2_balance_lp / lp_mint_supply) as u64;
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
                mint: *inner_ctx.mint_a.key(),
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
                mint: *inner_ctx.mint_b.key(),
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

    // Carry out the actual deposit logic
    //  CPI'ing into the SPL Token Swap program
    if amount_a > 0 {
        deposit_single_token_type_exact_amount_in_cpi(
            amount_a,
            Signer::from(&[
                Seed::from(CONTROLLER_SEED),
                Seed::from(&controller_id_bytes),
                Seed::from(&[controller_bump]),
            ]),
            *inner_ctx.swap_program.key(),
            inner_ctx.swap,
            inner_ctx.swap_authority,
            outer_ctx.controller,
            inner_ctx.vault_a,
            inner_ctx.swap_token_a,
            inner_ctx.swap_token_b,
            inner_ctx.lp_mint,
            inner_ctx.lp_token_account,
            inner_ctx.mint_a,
            inner_ctx.mint_a_token_program,
            inner_ctx.lp_mint_token_program,
        )?;
    }
    if amount_b > 0 {
        deposit_single_token_type_exact_amount_in_cpi(
            amount_b,
            Signer::from(&[
                Seed::from(CONTROLLER_SEED),
                Seed::from(&controller_id_bytes),
                Seed::from(&[controller_bump]),
            ]),
            *inner_ctx.swap_program.key(),
            inner_ctx.swap,
            inner_ctx.swap_authority,
            outer_ctx.controller,
            inner_ctx.vault_b,
            inner_ctx.swap_token_a,
            inner_ctx.swap_token_b,
            inner_ctx.lp_mint,
            inner_ctx.lp_token_account,
            inner_ctx.mint_b,
            inner_ctx.mint_b_token_program,
            inner_ctx.lp_mint_token_program,
        )?;
    }

    // Refresh values for LP Mint supply, LP tokens held
    //  and swap pool owned balances for tokens a and b
    let lp_token_account = TokenAccount::from_account_info(inner_ctx.lp_token_account)?;
    let post_deposit_balance_lp = lp_token_account.amount() as u128;
    drop(lp_token_account);

    let lp_mint = Mint::from_account_info(inner_ctx.lp_mint).unwrap();
    let lp_mint_supply = lp_mint.supply() as u128;
    let swap_token_a = TokenAccount::from_account_info(inner_ctx.swap_token_a)?;
    let swap_token_b = TokenAccount::from_account_info(inner_ctx.swap_token_b)?;
    let delta_lp = post_deposit_balance_lp
        .checked_sub(step_2_balance_lp)
        .unwrap();

    // Determine the share of the pool's a and b tokens that we have a claim on
    let post_deposit_balance_a: u64;
    let post_deposit_balance_b: u64;
    if post_deposit_balance_lp > 0 {
        post_deposit_balance_a =
            (swap_token_a.amount() as u128 * post_deposit_balance_lp / lp_mint_supply) as u64;
        post_deposit_balance_b =
            (swap_token_b.amount() as u128 * post_deposit_balance_lp / lp_mint_supply) as u64;
    } else {
        post_deposit_balance_a = 0u64;
        post_deposit_balance_b = 0u64;
    }
    drop(swap_token_a);
    drop(swap_token_b);
    msg! {"f"};

    // Emit the accounting event
    if step_2_balance_a != post_deposit_balance_a {
        controller.emit_event(
            outer_ctx.controller,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: *inner_ctx.mint_a.key(),
                action: AccountingAction::Deposit,
                before: step_2_balance_a,
                after: post_deposit_balance_a,
            }),
        )?;
    }
    // Emit the accounting event
    if step_2_balance_b != post_deposit_balance_b {
        controller.emit_event(
            outer_ctx.controller,
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: *inner_ctx.mint_b.key(),
                action: AccountingAction::Deposit,
                before: step_2_balance_b,
                after: post_deposit_balance_b,
            }),
        )?;
    }
    msg! {"g"};

    // Update the state for the changes in balances
    match &mut integration.state {
        IntegrationState::SplTokenSwap(state) => {
            state.last_balance_a = post_deposit_balance_a;
            state.last_balance_b = post_deposit_balance_b;
            state.last_balance_lp = post_deposit_balance_lp as u64;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    // Update the integration rate limit for the outflow
    //  Rate limit for the SplTokenSwap is (counterintuitively) tracked in
    //  units of LP tokens (in, for tokens a or b out)
    integration.update_rate_limit_for_outflow(clock, delta_lp as u64)?;

    // Update the reserves for the flows
    if amount_a > 0 {
        reserve_a.update_for_outflow(clock, amount_a)?;
    }
    if amount_b > 0 {
        reserve_b.update_for_outflow(clock, amount_b)?;
    }

    Ok(())
}
