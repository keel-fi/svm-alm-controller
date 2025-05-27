use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_token::state::TokenAccount;

use crate::{
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    instructions::AtomicSwapBorrowArgs,
    state::{nova_account::NovaAccount, Controller, Integration, Permission, Reserve},
};

pub struct AtomicSwapBorrow<'info> {
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub reserve_a: &'info AccountInfo,
    pub vault_a: &'info AccountInfo,
    pub reserve_b: &'info AccountInfo,
    pub vault_b: &'info AccountInfo,
    pub recipient_token_account: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
}

impl<'info> AtomicSwapBorrow<'info> {
    // TODO: Let Reserve be mutable to enforce rate limits?
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller: &accounts[0],
            authority: &accounts[1],
            permission: &accounts[2],
            integration: &accounts[3],
            reserve_a: &accounts[4],
            vault_a: &accounts[5],
            reserve_b: &accounts[6],
            vault_b: &accounts[7],
            recipient_token_account: &accounts[8],
            token_program: &accounts[9],
        };
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.integration.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !ctx.vault_a.is_writable() {
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
        Ok(ctx)
    }
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

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    // TODO: Verify that this is the right permission to check
    if !permission.can_execute_swap() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Check that mint and vault account matches known keys in controller-associated Reserve.
    let reserve_a = Reserve::load_and_check(ctx.reserve_a, ctx.controller.key())?;
    if reserve_a.vault != *ctx.vault_a.key() {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }
    let reserve_b = Reserve::load_and_check(ctx.reserve_b, ctx.controller.key())?;
    if reserve_b.vault != *ctx.vault_b.key() {
        return Err(SvmAlmControllerErrors::InvalidAccountData.into());
    }

    let controller = Controller::load_and_check(ctx.controller)?;

    // Check that Integration account is valid and matches controller.
    let mut integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;

    if let (IntegrationConfig::AtomicSwap(cfg), IntegrationState::AtomicSwap(state)) =
        (&integration.config, &mut integration.state)
    {
        if cfg.input_token != reserve_a.mint || cfg.output_token != reserve_b.mint {
            return Err(SvmAlmControllerErrors::InvalidAccountData.into());
        }

        if state.has_swap_started() {
            return Err(SvmAlmControllerErrors::SwapHasStarted.into());
        }

        {
            let vault_a = TokenAccount::from_account_info(ctx.vault_a)?;
            let vault_b = TokenAccount::from_account_info(ctx.vault_b)?;

            if args.amount > vault_a.amount() {
                return Err(ProgramError::InsufficientFunds);
            }
            if args.amount == 0 {
                return Err(ProgramError::InvalidArgument);
            }

            // Cache vault balances and amount borrowed in Integration state.
            state.last_balance_a = vault_a.amount();
            state.last_balance_b = vault_b.amount();
            state.amount_borrowed = args.amount;
            integration.save(ctx.integration)?;
        }

        // Transfer bprrow amount of tokens from vault to recipient.
        controller.transfer_tokens(
            ctx.controller,
            ctx.vault_a,
            ctx.recipient_token_account,
            args.amount,
        )?;
    } else {
        return Err(SvmAlmControllerErrors::Invalid.into());
    }

    // TODO: Uses transaction introspection to confirm that the last ixn in the txn is SwapRepay.
    // Needs to check that its SwapRepay for the same integration.
    Ok(())
}
