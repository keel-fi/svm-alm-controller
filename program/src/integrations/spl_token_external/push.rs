use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, 
    msg, 
    program_error::ProgramError, 
    sysvars::{clock::Clock, Sysvar} 
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use crate::{
    constants::CONTROLLER_SEED, 
    enums::{IntegrationConfig, IntegrationState}, 
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent}, 
    instructions::PushArgs, 
    processor::{shared::emit_cpi, PushAccounts}, 
    state::{Controller, Integration, Permission} 
};
use pinocchio_token::{
    self, 
    instructions::Transfer, 
    state::TokenAccount
};


pub struct PushSplTokenExternalAccounts<'info> {
    pub spl_token_vault_integration: &'info AccountInfo,
    pub mint: &'info AccountInfo,
    pub vault: &'info AccountInfo,
    pub recipient: &'info AccountInfo,
    pub recipient_token_account: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> PushSplTokenExternalAccounts<'info> {

    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 8 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            spl_token_vault_integration: &account_infos[0],
            mint: &account_infos[1],
            vault: &account_infos[2],
            recipient: &account_infos[3],
            recipient_token_account: &account_infos[4],
            token_program: &account_infos[5],
            associated_token_program: &account_infos[6],
            system_program: &account_infos[7]
        };
        let config = match config {
            IntegrationConfig::SplTokenExternal(config) => config,
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
        if ctx.recipient.key().ne(&config.recipient) {
            msg!{"recipient: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.recipient_token_account.key().ne(&config.token_account) {
            msg!{"recipient_token_account: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.recipient_token_account.is_writable() {
            msg!{"recipient_token_account: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.recipient_token_account.owner().ne(ctx.token_program.key()) && ctx.recipient_token_account.owner().ne(&pinocchio_system::ID) {
            msg!{"recipient_token_account: not owned by token_program or system_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.token_program.key().ne(&config.program) { // TODO: Allow token 2022
            msg!{"token_program: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.associated_token_program.key().ne(&pinocchio_associated_token_account::ID) { 
            msg!{"associated_token_program: invalid address"};
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




pub fn process_push_spl_token_external(
    controller: &Controller,
    permission: &Permission,
    integration: &Integration,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs
) -> Result<(), ProgramError> {
    
    // SplTokenExternal PUSH implementation

    msg!("process_push_spl_token_external");

    // Get the current slot and time
    let clock = Clock::get()?;

    let amount = match outer_args {
        PushArgs::SplTokenExternal { amount } => { *amount },
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

    let inner_ctx = PushSplTokenExternalAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts
    )?;

    // Load corresponding SplTokenVault integration 
    let mut spl_token_vault_integration = Integration::load_and_check(
        inner_ctx.spl_token_vault_integration, 
        outer_ctx.controller_info.key(), 
    )?;

    // CHeck consistency between the SplTokenVault integration's config and the 
    //  SplTokenExternal integrations config
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

    // Perform a SYNC on the 
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
    
    // Invoke the CreateIdempotent ixn 
    CreateIdempotent{
        funding_account: outer_ctx.authority_info,
        account: inner_ctx.recipient_token_account,
        wallet: inner_ctx.recipient,
        mint: inner_ctx.mint,
        system_program: inner_ctx.system_program,
        token_program: inner_ctx.token_program,
    }.invoke()?;


    // Perform the transfer
    Transfer{
        from: inner_ctx.vault,
        to: inner_ctx.recipient_token_account,
        authority: outer_ctx.controller_info,
        amount: amount,
    }.invoke_signed(
        &[
            Signer::from(
                &[
                    Seed::from(CONTROLLER_SEED),
                    Seed::from(&controller_id_bytes),
                    Seed::from(&[controller_bump])
                ]
            )
        ]
    )?;
    


    // Reload the vault account to check it's balance
    let vault = TokenAccount::from_account_info(&inner_ctx.vault)?;
    let post_transfer_balance = vault.amount();
    let check_delta = post_sync_balance.checked_sub(post_transfer_balance).unwrap();
    if check_delta != amount {
        msg!{"check_delta: transfer did not match the vault balance change"};
        return Err(ProgramError::InvalidArgument);
    }

    msg!("after checks");

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
                action: AccountingAction::ExternalTransfer,
                before: post_sync_balance,
                after: post_transfer_balance
            }
        )
    )?;

    
    // Save the changes to the SplTokenVault integration account
    spl_token_vault_integration.save(&inner_ctx.spl_token_vault_integration)?;

    // Save the changes to the SplTokenExternal integration account
    integration.save(&outer_ctx.integration_info)?;

    
    Ok(())

}

