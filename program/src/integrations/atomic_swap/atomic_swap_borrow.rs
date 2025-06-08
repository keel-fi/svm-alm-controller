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
use pinocchio_token::state::TokenAccount;

use crate::{
    constants::{ATOMIC_SWAP_REPAY_INTEGRATION_IDX, ATOMIC_SWAP_REPAY_IX_DISC},
    enums::{
        ControllerStatus, IntegrationConfig, IntegrationState, IntegrationStatus, ReserveStatus,
    },
    error::SvmAlmControllerErrors,
    instructions::AtomicSwapBorrowArgs,
    state::{nova_account::NovaAccount, Integration, Permission, Reserve},
    wrapper::{
        ControllerAccount, PermissionAccount, PermissionArgs, ReserveAccount, ReserveArgs,
        WrappedAccount,
    },
};

pub struct AtomicSwapBorrow<'info> {
    pub controller: ControllerAccount<'info>,
    pub authority: &'info AccountInfo,
    pub permission: PermissionAccount<'info>,
    pub integration: &'info AccountInfo,
    pub reserve_a: &'info mut ReserveAccount<'info>,
    pub vault_a: &'info AccountInfo,
    pub reserve_b: &'info ReserveAccount<'info>,
    pub vault_b: &'info AccountInfo,
    pub recipient_token_account: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
    pub sysvar_instruction: &'info AccountInfo,
}

impl<'info> AtomicSwapBorrow<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 11 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let controller = ControllerAccount::new(&accounts[0])?;
        let controller_key = controller.info().key();
        let authority = &accounts[0];
        let ctx = Self {
            controller,
            authority: &accounts[1],
            permission: PermissionAccount::new_with_args(
                &accounts[2],
                PermissionArgs {
                    controller: controller_key,
                    authority: authority.key(),
                },
            )?,
            integration: &accounts[3],
            reserve_a: &mut ReserveAccount::new_with_args_mut(
                &accounts[4],
                ReserveArgs {
                    controller: controller_key,
                },
            )?,
            vault_a: &accounts[5],
            reserve_b: &mut ReserveAccount::new_with_args_mut(
                &accounts[6],
                ReserveArgs {
                    controller: controller_key,
                },
            )?,
            vault_b: &accounts[7],
            recipient_token_account: &accounts[8],
            token_program: &accounts[9],
            sysvar_instruction: &accounts[10],
        };
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.integration.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !ctx.reserve_a.info().is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !ctx.vault_a.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !ctx.reserve_b.info().is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !ctx.recipient_token_account.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if ctx.token_program.key().ne(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.sysvar_instruction.key().ne(&INSTRUCTIONS_ID) {
            msg! {"sysvar_instruction: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(ctx)
    }
}

/// Checks that repay ix for the same atomic swap is the last instruction in the same transaction.
pub fn verify_repay_ix_in_tx(
    sysvar_instruction: &AccountInfo,
    integration: &Pubkey,
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

    Ok(())
}

pub fn process_atomic_swap_borrow(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("atomic_swap_borrow");
    let ctx = AtomicSwapBorrow::from_accounts(accounts)?;
    let args: AtomicSwapBorrowArgs =
        AtomicSwapBorrowArgs::try_from_slice(instruction_data).unwrap();

    let controller = ctx.controller.inner();
    let permission = ctx.permission.inner();
    let reserve_a = ctx.reserve_a.inner();
    let reserve_b = ctx.reserve_b.inner();

    // Load in controller state
    if controller.status != ControllerStatus::Active {
        return Err(SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction.into());
    }

    // Check permission
    if !permission.can_execute_swap() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    let clock = Clock::get()?;

    // Check that mint and vault account matches known keys in controller-associated Reserve.
    if reserve_a.vault != *ctx.vault_a.key() {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    if reserve_a.status != ReserveStatus::Active {
        return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
    }

    if reserve_b.vault != *ctx.vault_b.key() {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    if reserve_b.status != ReserveStatus::Active {
        return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
    }

    // Sync reserve balances and rate limits
    reserve_a.sync_balance(ctx.vault_a, ctx.controller.info(), controller)?;
    reserve_b.sync_balance(ctx.vault_b, ctx.controller.info(), controller)?;

    // Check that Integration account is valid and matches controller.
    let mut integration = Integration::load_and_check_mut(ctx.integration, ctx.controller.key())?;
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
            let recipient_token_account =
                TokenAccount::from_account_info(ctx.recipient_token_account)?;

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
            state.recipient_token_a_pre = recipient_token_account.amount();
            state.repay_excess_token_a = args.repay_excess_token_a;
        }

        // Transfer borrow amount of tokens from vault to recipient.
        controller.transfer_tokens(
            ctx.controller.info(),
            ctx.vault_a,
            ctx.recipient_token_account,
            args.amount,
        )?;
    } else {
        return Err(SvmAlmControllerErrors::Invalid.into());
    }

    verify_repay_ix_in_tx(ctx.sysvar_instruction, ctx.integration.key())?;

    reserve_a.update_for_outflow(clock, args.amount)?;
    reserve_a.save(ctx.reserve_a.info())?;
    reserve_b.save(ctx.reserve_b.info())?;

    // Update rate limit to track outflow of input_tokens for integration.
    integration.update_rate_limit_for_outflow(clock, args.amount)?;
    integration.save(ctx.integration)?;

    Ok(())
}
