use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    error::SvmAlmControllerErrors,
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::lz_bridge::{config::LzBridgeConfig, cpi::OftSendParams},
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};
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
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::{self, state::TokenAccount};

define_account_struct! {
    pub struct PushLzBridgeAccounts<'info> {
        mint;
        vault;
        authority_token_account;
        token_program: @pubkey(pinocchio_token::ID); // TODO: Allow token 2022
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
        system_program: @pubkey(pinocchio_system::ID);
        sysvar_instruction: @pubkey(INSTRUCTIONS_ID);
    }
}

impl<'info> PushLzBridgeAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::LzBridge(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if ctx.mint.key().ne(&config.mint) {
            msg! {"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

/// Checks that LZ OFT send ix is the last instruction in the same transaction.
pub fn verify_send_ix_in_tx(
    authority: &Pubkey,
    accounts: &PushLzBridgeAccounts,
    config: &LzBridgeConfig,
    amount: u64,
) -> ProgramResult {
    // Get number of instructions in current transaction.
    let sysvar_data = accounts.sysvar_instruction.try_borrow_data()?;
    if sysvar_data.len() < 2 {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }
    let ix_len = u16::from_le_bytes([sysvar_data[0], sysvar_data[1]]);

    let instructions = Instructions::try_from(accounts.sysvar_instruction)?;

    // Check that current ix is before the last ix.
    let curr_ix = instructions.load_current_index();
    if curr_ix >= ix_len - 1 {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Load last instruction in transaction and check that its for OFT program.
    let last_ix = instructions.load_instruction_at((ix_len - 1).into())?;
    if last_ix.get_program_id().ne(&config.program) {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }

    // Deserializes and checks that ix discriminator matches known send_ix discriminator.
    let send_args = OftSendParams::deserialize(last_ix.get_instruction_data())?;

    let signer = last_ix.get_account_meta_at(0)?.key;
    let peer_config = last_ix.get_account_meta_at(1)?.key;
    let oft_store = last_ix.get_account_meta_at(2)?.key;
    let token_source = last_ix.get_account_meta_at(3)?.key;
    let token_escrow = last_ix.get_account_meta_at(4)?.key;
    let token_mint = last_ix.get_account_meta_at(5)?.key;
    let token_program = last_ix.get_account_meta_at(6)?.key;

    // Check that accounts for send_ix matches known accounts.
    if signer.ne(authority)
        || peer_config.ne(&config.peer_config)
        || oft_store.ne(&config.oft_store)
        || token_source.ne(accounts.authority_token_account.key())
        || token_escrow.ne(&config.token_escrow)
        || token_mint.ne(accounts.mint.key())
        || token_program.ne(accounts.token_program.key())
    {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }

    // Check that ix args for send_ix matches known values.
    if send_args.amount_ld != amount
        || send_args.to != config.destination_address
        || send_args.dst_eid != config.destination_eid
    {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }

    Ok(())
}

pub fn process_push_lz_bridge(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    msg!("process_push_lz_bridge");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PushArgs::LzBridge { amount } => *amount,
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

    let inner_ctx = PushLzBridgeAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    // Load the destination_address and destination_domain from the config
    let config = match integration.config {
        IntegrationConfig::LzBridge(config) => config,
        _ => return Err(ProgramError::InvalidAccountData),
    };
    verify_send_ix_in_tx(outer_ctx.authority.key(), &inner_ctx, &config, amount)?;

    // Check against reserve data
    if inner_ctx.vault.key().ne(&reserve.vault) {
        msg! {"vault: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }
    if inner_ctx.mint.key().ne(&reserve.mint) {
        msg! {"mint: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Sync the balance before doing anything else
    reserve.sync_balance(inner_ctx.vault, outer_ctx.controller, controller)?;
    let post_sync_balance = reserve.last_balance;

    // Creates the authority token_account, if necessary, or validates it
    CreateIdempotent {
        funding_account: outer_ctx.authority,
        account: inner_ctx.authority_token_account,
        wallet: outer_ctx.authority,
        mint: inner_ctx.mint,
        system_program: inner_ctx.system_program,
        token_program: inner_ctx.token_program,
    }
    .invoke()?;

    // Transfer the token to the token destination, where the t
    // from here the token will be burned or locked in the OFT Send instruction
    controller.transfer_tokens(
        outer_ctx.controller,
        outer_ctx.controller_authority,
        inner_ctx.vault,
        inner_ctx.authority_token_account,
        amount,
    )?;

    /////////

    // Reload the vault account to check it's balance
    let vault = TokenAccount::from_account_info(&inner_ctx.vault)?;
    let post_transfer_balance = vault.amount();
    let check_delta = post_sync_balance
        .checked_sub(post_transfer_balance)
        .unwrap();
    if check_delta != amount {
        msg! {"check_delta: transfer did not match the vault balance change"};
        return Err(ProgramError::InvalidArgument);
    }

    // Update the rate limit for the outflow
    integration.update_rate_limit_for_outflow(clock, amount)?;

    // No state transitions for LzBridge

    // Update the reserve for the outflow
    reserve.update_for_outflow(clock, amount)?;

    // Emit the accounting event
    controller.emit_event(
        outer_ctx.controller,
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: *outer_ctx.integration.key(),
            mint: *inner_ctx.mint.key(),
            action: AccountingAction::BridgeSend,
            before: post_sync_balance,
            after: post_transfer_balance,
        }),
    )?;

    Ok(())
}
