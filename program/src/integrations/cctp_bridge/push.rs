use crate::{
    constants::CONTROLLER_SEED,
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
use pinocchio_token::{self, state::TokenAccount};

pub struct PushCctpBridgeAccounts<'info> {
    pub mint: &'info AccountInfo,
    pub vault: &'info AccountInfo,
    pub sender_authority_pda: &'info AccountInfo,
    pub message_transmitter: &'info AccountInfo,
    pub token_messenger: &'info AccountInfo,
    pub remote_token_messenger: &'info AccountInfo,
    pub token_minter: &'info AccountInfo,
    pub local_token: &'info AccountInfo,
    pub message_sent_event_data: &'info AccountInfo,
    pub cctp_message_transmitter: &'info AccountInfo,
    pub cctp_token_messenger_minter: &'info AccountInfo,
    pub event_authority: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> PushCctpBridgeAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 14 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            mint: &account_infos[0],
            vault: &account_infos[1],
            sender_authority_pda: &account_infos[2],
            message_transmitter: &account_infos[3],
            token_messenger: &account_infos[4],
            remote_token_messenger: &account_infos[5],
            token_minter: &account_infos[6],
            local_token: &account_infos[7],
            message_sent_event_data: &account_infos[8],
            cctp_message_transmitter: &account_infos[9],
            cctp_token_messenger_minter: &account_infos[10],
            event_authority: &account_infos[11],
            token_program: &account_infos[12],
            system_program: &account_infos[13],
        };
        let config = match config {
            IntegrationConfig::CctpBridge(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if ctx.mint.key().ne(&config.mint) {
            msg! {"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.mint.is_owned_by(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"mint: not owned by token_program"};
            return Err(ProgramError::InvalidAccountOwner);
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
        if ctx.mint.key().ne(&config.mint) {
            msg! {"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.token_program.key().ne(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.system_program.key().ne(&pinocchio_system::ID) {
            msg! {"system_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
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
    if !permission.can_invoke_external_transfer() {
        msg! {"permission: can_invoke_external_transfer required"};
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
    let local_mint =
        LocalToken::deserialize(&mut &*inner_ctx.local_token.try_borrow_data()?).unwrap();
    if local_mint.mint.ne(inner_ctx.mint.key()) {
        msg! {"mint: does not match local_mint state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Load in the CCTP RemoteTokenMessenger account and verify the mint matches
    let remote_token_messenger = RemoteTokenMessenger::deserialize(
        &mut &*inner_ctx.remote_token_messenger.try_borrow_data()?,
    )
    .unwrap();
    if remote_token_messenger.domain.ne(&destination_domain) {
        msg! {"desination_domain: does not match remote_token_messenger state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Check against reserve data
    if inner_ctx.vault.key().ne(&reserve.vault) {
        msg! {"mint: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }
    if inner_ctx.mint.key().ne(&reserve.mint) {
        msg! {"mint: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Sync the balance before doing anything else
    reserve.sync_balance(inner_ctx.vault, outer_ctx.controller, controller)?;
    let post_sync_balance = reserve.last_balance;

    let controller_id_bytes = controller.id.to_le_bytes();
    let controller_bump = controller.bump;

    // Perform the CPI to deposit and burn
    deposit_for_burn_cpi(
        amount,
        destination_domain,
        destination_address,
        Signer::from(&[
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller_id_bytes),
            Seed::from(&[controller_bump]),
        ]),
        outer_ctx.controller,
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
