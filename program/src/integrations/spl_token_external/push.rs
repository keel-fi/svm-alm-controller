use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_log::log;
use pinocchio_token_interface::{Mint, TokenAccount};

define_account_struct! {
  pub struct PushSplTokenExternalAccounts<'info> {
      mint;
      vault: mut;
      recipient;
      recipient_token_account: mut;
      token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
      associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
      system_program: @pubkey(pinocchio_system::ID);
  }
}

impl<'info> PushSplTokenExternalAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::SplTokenExternal(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if ctx.mint.key().ne(&config.mint) {
            msg! {"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.mint.is_owned_by(&config.program) {
            msg! {"mint: not owned by token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.recipient.key().ne(&config.recipient) {
            msg! {"recipient: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.recipient_token_account.key().ne(&config.token_account) {
            msg! {"recipient_token_account: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx
            .recipient_token_account
            .is_owned_by(ctx.token_program.key())
            && !ctx
                .recipient_token_account
                .is_owned_by(&pinocchio_system::ID)
        {
            msg! {"recipient_token_account: not owned by token_program or system_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.token_program.key().ne(&config.program) {
            msg! {"token_program: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(ctx)
    }
}

pub fn process_push_spl_token_external(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    // SplTokenExternal PUSH implementation

    msg!("process_push_spl_token_external");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PushArgs::SplTokenExternal { amount } => *amount,
        _ => return Err(ProgramError::InvalidAccountData),
    };
    if amount == 0 {
        msg! {"amount: must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    // Check permission
    if !permission.can_invoke_external_transfer() {
        msg! {"permission: can_invoke_external_transfer required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushSplTokenExternalAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    // Check consistency between the reserve
    //  SplTokenExternal integrations config
    if inner_ctx.vault.key().ne(&reserve.vault) {
        msg! {"vault: does not match config"};
        return Err(ProgramError::InvalidAccountData);
    }
    if inner_ctx.mint.key().ne(&reserve.mint) {
        msg! {"mint: mismatch between integration configs"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Sync the reserve before main logic
    reserve.sync_balance(
        inner_ctx.vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;
    let post_sync_balance = reserve.last_balance;

    // Invoke the CreateIdempotent ixn
    //  to create or validate the ATA for the recipient
    //  external account
    CreateIdempotent {
        funding_account: outer_ctx.authority,
        account: inner_ctx.recipient_token_account,
        wallet: inner_ctx.recipient,
        mint: inner_ctx.mint,
        system_program: inner_ctx.system_program,
        token_program: inner_ctx.token_program,
    }
    .invoke()?;

    // Perform the transfer
    let mint = Mint::from_account_info(&inner_ctx.mint)?;
    controller.transfer_tokens(
        outer_ctx.controller,
        outer_ctx.controller_authority,
        inner_ctx.vault,
        inner_ctx.recipient_token_account,
        inner_ctx.mint,
        amount,
        mint.decimals(),
        inner_ctx.token_program.key(),
    )?;

    // Reload the vault account to check it's balance
    let vault = TokenAccount::from_account_info(&inner_ctx.vault)?;
    let post_transfer_balance = vault.amount();
    log!("post_sync_balance: {}", post_sync_balance);
    log!("post_transfer_balance: {}", post_transfer_balance);
    let check_delta = post_sync_balance
        .checked_sub(post_transfer_balance)
        .unwrap();
    if check_delta != amount {
        msg! {"check_delta: transfer did not match the vault balance change"};
        return Err(ProgramError::InvalidArgument);
    }

    // Update the rate limit for the outflow
    integration.update_rate_limit_for_outflow(clock, amount)?;

    // No state transitions for SplTokenExternal

    // Update reserve balance and rate limits for the outflow
    reserve.update_for_outflow(clock, amount, false)?;

    // Emit the accounting event
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: None,
            reserve: Some(*outer_ctx.reserve_a.key()),
            mint: *inner_ctx.mint.key(),
            action: AccountingAction::ExternalTransfer,
            before: post_sync_balance,
            after: post_transfer_balance,
        }),
    )?;

    Ok(())
}
