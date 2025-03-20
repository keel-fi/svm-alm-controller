use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, 
    msg, 
    program_error::ProgramError, 
    sysvars::{clock::Clock, Sysvar} 
};
use crate::{
    constants::CONTROLLER_SEED, 
    enums::{IntegrationConfig, IntegrationState}, 
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent}, 
    instructions::PushArgs, 
    integrations::cctp_bridge::{
        cctp_state::{LocalToken, RemoteTokenMessenger}, 
        cpi::deposit_for_burn_cpi
    }, 
    processor::{shared::emit_cpi, PushAccounts}, 
    state::{Controller, Integration, Permission} 
};
use pinocchio_token::{
    self, 
    state::TokenAccount
};
use borsh::BorshDeserialize;


pub struct PushCctpBridgeAccounts<'info> {
    pub spl_token_vault_integration: &'info AccountInfo,
    pub mint: &'info AccountInfo,
    pub vault: &'info AccountInfo,
    pub sender_authority_pda: &'info AccountInfo,
    pub message_transmitter: &'info AccountInfo,
    pub token_messenger: &'info AccountInfo,
    pub remote_token_messenger: &'info AccountInfo,
    pub token_minter: &'info AccountInfo,
    pub local_token: &'info AccountInfo,
    pub message_sent_event_data: &'info AccountInfo,
    pub message_transmitter_program: &'info AccountInfo,
    pub token_messenger_minter_program: &'info AccountInfo,
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
            spl_token_vault_integration: &account_infos[0],
            mint: &account_infos[1],
            vault: &account_infos[2],
            sender_authority_pda: &account_infos[3],
            message_transmitter: &account_infos[4],
            token_messenger: &account_infos[5],
            remote_token_messenger: &account_infos[6],
            token_minter: &account_infos[7],
            local_token: &account_infos[8],
            message_sent_event_data: &account_infos[9],
            message_transmitter_program: &account_infos[10],
            token_messenger_minter_program: &account_infos[11],
            token_program: &account_infos[12],
            system_program: &account_infos[13],
        };
        let config = match config {
            IntegrationConfig::CctpBridge(config) => config,
            _ => return Err(ProgramError::InvalidAccountData)
        };
        if ctx.mint.key().ne(&config.mint) { 
            msg!{"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.mint.owner().ne(&config.program) { 
            msg!{"mint: not owned by token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.token_messenger_minter_program.key().ne(&config.program) { // TODO: Allow token 2022
            msg!{"token_messenger_minter_program: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.mint.key().ne(&config.mint) { // TODO: Allow token 2022
            msg!{"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.token_program.key().ne(&config.program) { // TODO: Allow token 2022
            msg!{"token_program: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.system_program.key().ne(&pinocchio_system::ID) { 
            msg!{"system_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if !ctx.spl_token_vault_integration.is_writable() {
            msg!{"spl_token_vault_integration: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(ctx)
    }

}




pub fn process_push_cctp_bridge(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs
) -> Result<(), ProgramError> {
    
    // CctpBridge PUSH implementation

    msg!("process_push_cctp_bridge");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PushArgs::CctpBridge { amount } => { *amount },
        _ => return Err(ProgramError::InvalidAccountData)
    };
    if amount == 0 {
        msg!{"amount: must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check permission
    if !permission.can_invoke_external_transfer() {
        msg!{"permission: can_invoke_external_transfer required"};
        return Err(ProgramError::IncorrectAuthority)
    }

    let inner_ctx = PushCctpBridgeAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts
    )?;

    // Load corresponding SplTokenVault integration 
    let mut spl_token_vault_integration = Integration::load_and_check(
        inner_ctx.spl_token_vault_integration, 
        outer_ctx.controller_info.key(), 
    )?;

    // Load the destination_address and destination_domain from the config
    let (destination_address, destination_domain) = match integration.config {
        IntegrationConfig::CctpBridge(config) => {
            (config.destination_address, config.destination_domain)
        },
        _ => return Err(ProgramError::InvalidAccountData)
    };

    // Load in the CCTP Local Token Account and verify the mint matches
    let local_mint= LocalToken::deserialize(&mut &*inner_ctx.local_token.try_borrow_data()?).unwrap();
    if local_mint.mint.ne(inner_ctx.mint.key()) {
        msg!{"mint: does not match local_mint state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Load in the CCTP RemoteTokenMessenger account and verify the mint matches
    let remote_token_messenger= RemoteTokenMessenger::deserialize(&mut &*inner_ctx.remote_token_messenger.try_borrow_data()?).unwrap();
    if remote_token_messenger.domain.ne(&destination_domain) {
        msg!{"desination_domain: does not match remote_token_messenger state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // CHeck consistency between the SplTokenVault integration's config and the 
    //  CctpBridge integrations config
    match spl_token_vault_integration.config {
        IntegrationConfig::SplTokenVault(spl_token_vault_config) => {
            if inner_ctx.vault.key().ne(&spl_token_vault_config.vault) { 
                msg!{"vault: does not match config"};
                return Err(ProgramError::InvalidAccountData);
            }
            if !inner_ctx.vault.is_writable() { 
                msg!{"vault: not mutable"};
                return Err(ProgramError::InvalidAccountData);
            }
            if inner_ctx.vault.owner().ne(&spl_token_vault_config.program) { 
                msg!{"vault: not owned by token_program"};
                return Err(ProgramError::InvalidAccountOwner);
            }
            if inner_ctx.mint.key().ne(&spl_token_vault_config.mint) { 
                msg!{"mint: mismatch between integration configs"};
                return Err(ProgramError::InvalidAccountData);
            }
            if inner_ctx.token_program.key().ne(&spl_token_vault_config.program) { 
                msg!{"token_program: mismatch between integration configs"};
                return Err(ProgramError::InvalidAccountData);
            }
        },
        _=> {
            msg!{"spl_token_vault_integration: wrong integration account type"};
            return Err(ProgramError::InvalidAccountData)
        }
    }

    // Perform a SYNC on the SPL Token Vault
    let vault = TokenAccount::from_account_info(&inner_ctx.vault)?;
    let starting_balance: u64;
    let post_sync_balance: u64;
    match &mut spl_token_vault_integration.state {
        IntegrationState::SplTokenVault(state) => {
            starting_balance = state.last_balance;
            post_sync_balance = vault.amount();
            state.last_refresh_timestamp = clock.unix_timestamp;
            state.last_refresh_slot = clock.slot;
            state.last_balance = post_sync_balance;
        },
        _ => return Err(ProgramError::InvalidAccountData.into())
    }
    drop(vault);

    let controller_id_bytes = controller.id.to_le_bytes();
    let controller_bump = controller.bump;
    if starting_balance != post_sync_balance {
        // Emit the accounting event
        emit_cpi(
            outer_ctx.controller_info,
            [
                Seed::from(CONTROLLER_SEED),
                Seed::from(&controller_id_bytes),
                Seed::from(&[controller_bump])
            ],
            SvmAlmControllerEvent::AccountingEvent (
                AccountingEvent {
                    controller: *outer_ctx.controller_info.key(),
                    integration: *inner_ctx.spl_token_vault_integration.key(),
                    mint: *inner_ctx.mint.key(),
                    action: AccountingAction::Sync,
                    before: starting_balance,
                    after: post_sync_balance
                }
            )
        )?;
    }
    
    // Perform the CPI to deposit and burn
    deposit_for_burn_cpi(
        amount, 
        destination_domain, 
        destination_address, 
        Signer::from(&[
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller_id_bytes),
            Seed::from(&[controller_bump])
        ]), 
        *inner_ctx.token_messenger_minter_program.key(), 
        outer_ctx.controller_info, 
        outer_ctx.authority_info, 
        inner_ctx.sender_authority_pda, 
        inner_ctx.vault, 
        inner_ctx.message_transmitter, 
        inner_ctx.token_messenger, 
        inner_ctx.remote_token_messenger, 
        inner_ctx.token_minter, 
        inner_ctx.local_token, 
        inner_ctx.mint, 
        inner_ctx.message_sent_event_data, 
        inner_ctx.message_transmitter_program, 
        inner_ctx.token_messenger_minter_program, 
        inner_ctx.token_program, 
        inner_ctx.system_program
    )?;
    

    // Reload the vault account to check it's balance
    let vault = TokenAccount::from_account_info(&inner_ctx.vault)?;
    let post_transfer_balance = vault.amount();
    let check_delta = post_sync_balance.checked_sub(post_transfer_balance).unwrap();
    if check_delta != amount {
        msg!{"check_delta: transfer did not match the vault balance change"};
        return Err(ProgramError::InvalidArgument);
    }


    // Update the vault integration state
    match &mut spl_token_vault_integration.state {
        IntegrationState::SplTokenVault(state) => {
            state.last_refresh_timestamp = clock.unix_timestamp;
            state.last_refresh_slot = clock.slot;
            state.last_balance = post_transfer_balance;
        },
        _ => return Err(ProgramError::InvalidAccountData.into())
    }


    // Emit the accounting event
    emit_cpi(
        outer_ctx.controller_info,
        [
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller_id_bytes),
            Seed::from(&[controller_bump])
        ],
        SvmAlmControllerEvent::AccountingEvent (
            AccountingEvent {
                controller: *outer_ctx.controller_info.key(),
                integration: *outer_ctx.integration_info.key(),
                mint: *inner_ctx.mint.key(),
                action: AccountingAction::BridgeSend,
                before: post_sync_balance,
                after: post_transfer_balance
            }
        )
    )?;

    
    // Save the changes to the SplTokenVault integration account
    spl_token_vault_integration.save(&inner_ctx.spl_token_vault_integration)?;

    // Save the changes to the CctpBridge integration account
    integration.save(&outer_ctx.integration_info)?;

    
    Ok(())

}

