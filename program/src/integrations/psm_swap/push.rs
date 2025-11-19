use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::psm_swap::{
        constants::PSM_SWAP_PROGRAM_ID, cpi::AddLiquidityToPsmToken, psm_swap_state::Token,
    },
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

define_account_struct! {
    pub struct PushPsmSwapAccounts<'info> {
        psm_pool: @owner(PSM_SWAP_PROGRAM_ID);
        psm_token: @owner(PSM_SWAP_PROGRAM_ID);
        psm_token_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
        psm_swap_program: @pubkey(PSM_SWAP_PROGRAM_ID);
    }
}

impl<'info> PushPsmSwapAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
        reserve: &Reserve,
    ) -> Result<Self, ProgramError> {
        let ctx = PushPsmSwapAccounts::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::PsmSwap(psm_config) => psm_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        // validate psm_pool matches config
        if config.psm_pool.ne(ctx.psm_pool.key()) {
            msg!("psm_pool: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        // validate psm_token matches config
        if config.psm_token.ne(ctx.psm_token.key()) {
            msg!("psm_token: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        // validate mint matches config
        if config.mint.ne(ctx.mint.key()) {
            msg!("mint: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }

        // validate reserve matches mint
        if reserve.mint.ne(ctx.mint.key()) {
            msg!("mint: does not match reserve");
            return Err(ProgramError::InvalidAccountData);
        }

        // validate reserve vault
        if ctx.reserve_vault.key().ne(&reserve.vault) {
            msg! {"reserve_vault: mismatch with reserve"};
            return Err(ProgramError::InvalidAccountData);
        }

        // validate token_program is correct
        if ctx.token_program.key().ne(ctx.mint.owner()) {
            msg! {"token_program: mismatch with mint"};
            return Err(ProgramError::InvalidAccountData);
        }

        // validate psm_token_vault matches the psm_token
        let psm_token_data = ctx.psm_token.try_borrow_data()?;
        let psm_token = Token::try_from_slice(&psm_token_data)?;

        if psm_token.vault.ne(ctx.psm_token_vault.key()) {
            msg! {"psm_token_vault: mismatch with psm_token"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

pub fn process_push_psm_swap(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    msg!("process_push_psm_swap");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PushArgs::PsmSwap { amount } => *amount,
        _ => return Err(ProgramError::InvalidArgument),
    };

    if amount == 0 {
        msg! {"amount must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    if !permission.can_reallocate() {
        msg! {"permission: can_reallocate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushPsmSwapAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
        reserve,
    )?;

    // sync reserve before CPI
    reserve.sync_balance(
        inner_ctx.reserve_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    let (liquidity_delta, direction) = {
        let prev_balance = match &integration.state {
            IntegrationState::PsmSwap(state) => state.liquidity_supplied,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let new_balance = TokenAccount::from_account_info(inner_ctx.psm_token_vault)?.amount();

        let abs_delta = new_balance.abs_diff(prev_balance);

        let direction = if new_balance > prev_balance {
            // liquidity increased
            AccountingDirection::Credit
        } else {
            // liquidity decreased
            AccountingDirection::Debit
        };

        (abs_delta, direction)
    };

    // sync integration before CPI if there was a change in liquidity provided
    if liquidity_delta > 0 {
        controller.emit_event(
            outer_ctx.controller_authority,
            outer_ctx.controller.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: Some(*outer_ctx.integration.key()),
                reserve: None,
                mint: *inner_ctx.mint.key(),
                action: AccountingAction::Sync,
                delta: liquidity_delta,
                direction,
            }),
        )?;
    }

    // CPI into PSM to add liquidity
    // NOTE: No need to manually calculate amount transferred
    // since PSM transfer the amount passed in args
    // and mint extensions with TransferFeeBps > 0 are disabled.
    AddLiquidityToPsmToken {
        payer: outer_ctx.controller_authority,
        psm_pool: inner_ctx.psm_pool,
        psm_token: inner_ctx.psm_token,
        mint: inner_ctx.mint,
        token_vault: inner_ctx.psm_token_vault,
        user_token_account: inner_ctx.reserve_vault,
        token_program: inner_ctx.token_program,
        associated_token_program: inner_ctx.associated_token_program,
        amount,
    }
    .invoke_signed(&[Signer::from(&[
        Seed::from(CONTROLLER_AUTHORITY_SEED),
        Seed::from(outer_ctx.controller.key()),
        Seed::from(&[controller.authority_bump]),
    ])])?;

    // Emit accounting event for credit Integration
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: Some(*outer_ctx.integration.key()),
            mint: *inner_ctx.mint.key(),
            reserve: None,
            direction: AccountingDirection::Credit,
            action: AccountingAction::Deposit,
            delta: amount,
        }),
    )?;

    // Emit accounting event for debit Reserve
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: None,
            mint: *inner_ctx.mint.key(),
            reserve: Some(*outer_ctx.reserve_a.key()),
            direction: AccountingDirection::Debit,
            action: AccountingAction::Deposit,
            delta: amount,
        }),
    )?;

    let total_liquidity_supplied =
        TokenAccount::from_account_info(inner_ctx.psm_token_vault)?.amount();

    // Update the state
    match &mut integration.state {
        IntegrationState::PsmSwap(state) => {
            state.liquidity_supplied = total_liquidity_supplied;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    // update the integration rate limit for outflow
    integration.update_rate_limit_for_outflow(clock, amount)?;

    // update the reserves for the flows
    reserve.update_for_outflow(clock, amount, false)?;

    Ok(())
}
