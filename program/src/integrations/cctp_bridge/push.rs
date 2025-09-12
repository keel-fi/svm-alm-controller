use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::IntegrationConfig,
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::cctp_bridge::{
        cctp_state::{LocalToken, RemoteTokenMessenger},
        cpi::deposit_for_burn_cpi,
    },
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_token_interface::TokenAccount;

define_account_struct! {
    pub struct PushCctpBridgeAccounts<'info> {
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        vault;
        sender_authority_pda;
        message_transmitter;
        token_messenger;
        remote_token_messenger;
        token_minter;
        local_token;
        message_sent_event_data: signer;
        cctp_message_transmitter;
        cctp_token_messenger_minter;
        event_authority;
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        system_program: @pubkey(pinocchio_system::ID);
    }
}

impl<'info> PushCctpBridgeAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = PushCctpBridgeAccounts::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::CctpBridge(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if ctx.mint.key().ne(&config.mint) {
            msg! {"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx
            .cctp_token_messenger_minter
            .key()
            .ne(&config.cctp_token_messenger_minter)
        {
            msg! {"cctp_token_messenger_minter: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx
            .cctp_message_transmitter
            .key()
            .ne(&config.cctp_message_transmitter)
        {
            msg! {"cctp_message_transmitter: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if !ctx
            .remote_token_messenger
            .is_owned_by(&config.cctp_token_messenger_minter)
        {
            msg! {"remote_token_messenger: invalid owner"};
            return Err(ProgramError::IllegalOwner);
        }
        if !ctx
            .local_token
            .is_owned_by(&config.cctp_token_messenger_minter)
        {
            msg! {"local_token: invalid owner"};
            return Err(ProgramError::IllegalOwner);
        }

        Ok(ctx)
    }
}

pub fn process_push_cctp_bridge(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    // CctpBridge PUSH implementation

    msg!("process_push_cctp_bridge");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PushArgs::CctpBridge { amount } => *amount,
        _ => return Err(ProgramError::InvalidAccountData),
    };
    if amount == 0 {
        msg! {"amount: must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    // Check permission
    if !permission.can_reallocate() && !permission.can_liquidate() {
        msg! {"permission: can_reallocate or can_liquidate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushCctpBridgeAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
    )?;

    // Load the destination_address and destination_domain from the config
    let (destination_address, destination_domain) = match integration.config {
        IntegrationConfig::CctpBridge(config) => {
            (config.destination_address, config.destination_domain)
        }
        _ => return Err(ProgramError::InvalidAccountData),
    };

    // Load in the CCTP Local Token Account and verify the mint matches
    let local_mint = LocalToken::deserialize(&mut &*inner_ctx.local_token.try_borrow_data()?)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    if local_mint.mint.ne(inner_ctx.mint.key()) {
        msg! {"mint: does not match local_mint state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Load in the CCTP RemoteTokenMessenger account
    let remote_token_messenger = RemoteTokenMessenger::deserialize(
        &mut &*inner_ctx.remote_token_messenger.try_borrow_data()?,
    )
    .map_err(|_| ProgramError::InvalidAccountData)?;
    if remote_token_messenger.domain.ne(&destination_domain) {
        msg! {"desination_domain: does not match remote_token_messenger state"};
        return Err(ProgramError::InvalidAccountData);
    }

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
    reserve.sync_balance(
        inner_ctx.vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;
    let post_sync_balance = reserve.last_balance;

    // Perform the CPI to deposit and burn
    deposit_for_burn_cpi(
        amount,
        destination_domain,
        destination_address,
        Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ]),
        outer_ctx.controller_authority,
        outer_ctx.authority,
        inner_ctx.sender_authority_pda,
        inner_ctx.vault,
        inner_ctx.message_transmitter,
        inner_ctx.token_messenger,
        inner_ctx.remote_token_messenger,
        inner_ctx.token_minter,
        inner_ctx.local_token,
        inner_ctx.mint,
        inner_ctx.message_sent_event_data,
        inner_ctx.cctp_message_transmitter,
        inner_ctx.cctp_token_messenger_minter,
        inner_ctx.event_authority,
        inner_ctx.token_program,
        inner_ctx.system_program,
    )?;

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

    // No state transitions for CctpBridge

    // Update the reserve for the outflow
    reserve.update_for_outflow(clock, amount, false)?;

    // Emit the accounting event for debit Reserve
    controller.emit_event(
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *outer_ctx.controller.key(),
            integration: None,
            reserve: Some(*outer_ctx.reserve_a.key()),
            mint: *inner_ctx.mint.key(),
            action: AccountingAction::BridgeSend,
            before: post_sync_balance,
            after: post_transfer_balance,
        }),
    )?;

    Ok(())
}
