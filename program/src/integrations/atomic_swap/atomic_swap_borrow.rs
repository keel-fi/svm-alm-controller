use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{
        clock::Clock,
        instructions::{Instructions, INSTRUCTIONS_ID},
        Sysvar,
    },
    ProgramResult,
};
use pinocchio_token_interface::{Mint, TokenAccount};

use crate::{
    constants::{
        ATOMIC_SWAP_REPAY_INTEGRATION_IDX, ATOMIC_SWAP_REPAY_IX_DISC,
        ATOMIC_SWAP_REPAY_PAYER_ACCOUNT_A_IDX, ATOMIC_SWAP_REPAY_PAYER_ACCOUNT_B_IDX,
    },
    define_account_struct,
    enums::{
        ControllerStatus, IntegrationConfig, IntegrationState, IntegrationStatus, ReserveStatus,
    },
    error::SvmAlmControllerErrors,
    instructions::AtomicSwapBorrowArgs,
    state::{keel_account::KeelAccount, Controller, Integration, Permission, Reserve},
};

define_account_struct! {
    pub struct AtomicSwapBorrow<'info> {
        controller;
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        permission;
        integration: mut;
        reserve_a: mut;
        vault_a: mut;
        mint_a;
        reserve_b: mut;
        vault_b;
        recipient_token_account_a: mut;
        recipient_token_account_b: mut;
        token_program_a: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        sysvar_instruction: @pubkey(INSTRUCTIONS_ID);
        program_id: @pubkey(crate::ID);
    }
}

/// Checks that repay ix for the same atomic swap is the last instruction in the same transaction.
pub fn verify_repay_ix_in_tx(
    sysvar_instruction: &AccountInfo,
    integration: &Pubkey,
    recipient_token_account_a: &Pubkey,
    recipient_token_account_b: &Pubkey,
) -> ProgramResult {
    // Get number of instructions in current transaction.
    let data = sysvar_instruction.try_borrow_data()?;
    if data.len() < 2 {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }
    let ix_len = u16::from_le_bytes([data[0], data[1]]);

    let instructions = Instructions::try_from(sysvar_instruction)?;

    // Check that current ix is before the last ix.
    let curr_ix = instructions.load_current_index();
    if curr_ix >= ix_len - 1 {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Load last instruction in transaction.
    let last_ix = instructions.load_instruction_at((ix_len - 1).into())?;
    if last_ix.get_program_id().ne(&crate::ID) {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }

    // Check that ix discriminator matches known atomic_swap_repay discriminator.
    let (discriminator, _) = last_ix
        .get_instruction_data()
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;
    if *discriminator != ATOMIC_SWAP_REPAY_IX_DISC {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }

    // Check that atomic_swap_repay is for the same integration account.
    let integration_acc =
        last_ix.get_account_meta_at(ATOMIC_SWAP_REPAY_INTEGRATION_IDX as usize)?;
    if integration_acc.key.ne(integration) {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }

    let payer_account_a =
        last_ix.get_account_meta_at(ATOMIC_SWAP_REPAY_PAYER_ACCOUNT_A_IDX as usize)?;
    let payer_account_b =
        last_ix.get_account_meta_at(ATOMIC_SWAP_REPAY_PAYER_ACCOUNT_B_IDX as usize)?;

    if payer_account_a.key.ne(recipient_token_account_a) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    if payer_account_b.key.ne(recipient_token_account_b) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }

    Ok(())
}

pub fn process_atomic_swap_borrow(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("atomic_swap_borrow");
    let ctx = AtomicSwapBorrow::from_accounts(accounts)?;
    let args: AtomicSwapBorrowArgs = AtomicSwapBorrowArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;
    if controller.status != ControllerStatus::Active {
        return Err(SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction.into());
    }

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    if !permission.can_execute_swap() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    let clock = Clock::get()?;

    // Check that mint and vault account matches known keys in controller-associated Reserve.
    let mut reserve_a = Reserve::load_and_check(ctx.reserve_a, ctx.controller.key())?;
    if reserve_a.vault.ne(ctx.vault_a.key()) || reserve_a.mint.ne(ctx.mint_a.key()) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    if reserve_a.status != ReserveStatus::Active {
        return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
    }

    let mut reserve_b = Reserve::load_and_check(ctx.reserve_b, ctx.controller.key())?;
    if reserve_b.vault.ne(ctx.vault_b.key()) {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    if reserve_b.status != ReserveStatus::Active {
        return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
    }

    // Sync reserve balances and rate limits
    reserve_a.sync_balance(
        ctx.vault_a,
        ctx.controller_authority,
        ctx.controller.key(),
        &controller,
    )?;
    reserve_b.sync_balance(
        ctx.vault_b,
        ctx.controller_authority,
        ctx.controller.key(),
        &controller,
    )?;

    // Check that Integration account is valid and matches controller.
    let mut integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;
    if integration.status != IntegrationStatus::Active {
        return Err(SvmAlmControllerErrors::IntegrationStatusDoesNotPermitAction.into());
    }
    integration.refresh_rate_limit(clock)?;

    if let (IntegrationConfig::AtomicSwap(cfg), IntegrationState::AtomicSwap(state)) =
        (&integration.config, &mut integration.state)
    {
        if cfg.input_token != reserve_a.mint || cfg.output_token != reserve_b.mint {
            return Err(SvmAlmControllerErrors::InvalidAccountData.into());
        }

        if state.has_swap_started() {
            return Err(SvmAlmControllerErrors::SwapHasStarted.into());
        }

        if clock.unix_timestamp >= cfg.expiry_timestamp {
            return Err(SvmAlmControllerErrors::IntegrationHasExpired.into());
        }

        {
            let vault_a = TokenAccount::from_account_info(ctx.vault_a)?;
            let vault_b = TokenAccount::from_account_info(ctx.vault_b)?;
            let recipient_token_a_account =
                TokenAccount::from_account_info(ctx.recipient_token_account_a)?;
            let recipient_token_b_account =
                TokenAccount::from_account_info(ctx.recipient_token_account_b)?;

            if args.amount > vault_a.amount() {
                return Err(ProgramError::InsufficientFunds);
            }
            if args.amount == 0 {
                return Err(ProgramError::InvalidArgument);
            }

            // Cache token balances and amount borrowed in Integration state.
            state.last_balance_a = vault_a.amount();
            state.last_balance_b = vault_b.amount();
            state.amount_borrowed = args.amount;
            state.recipient_token_a_pre = recipient_token_a_account.amount();
            state.recipient_token_b_pre = recipient_token_b_account.amount();
        }

        // Transfer borrow amount of tokens from vault to recipient.
        let mint_a = Mint::from_account_info(ctx.mint_a)?;
        controller.transfer_tokens(
            ctx.controller,
            ctx.controller_authority,
            ctx.vault_a,
            ctx.recipient_token_account_a,
            ctx.mint_a,
            args.amount,
            mint_a.decimals(),
            ctx.token_program_a.key(),
        )?;
    } else {
        return Err(SvmAlmControllerErrors::Invalid.into());
    }

    verify_repay_ix_in_tx(
        ctx.sysvar_instruction,
        ctx.integration.key(),
        ctx.recipient_token_account_a.key(),
        ctx.recipient_token_account_b.key(),
    )?;

    // NOTE: ok to use the amount from arguments as not Token Extension
    // configuration sends more or less than the requested amount.
    reserve_a.update_for_outflow(clock, args.amount, false)?;
    reserve_a.save(ctx.reserve_a)?;
    reserve_b.save(ctx.reserve_b)?;

    // Update rate limit to track outflow of input_tokens for integration.
    integration.update_rate_limit_for_outflow(clock, args.amount)?;
    integration.save(ctx.integration)?;

    Ok(())
}
