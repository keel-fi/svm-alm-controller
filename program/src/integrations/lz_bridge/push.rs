use crate::{
    enums::IntegrationConfig,
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::lz_bridge::cpi::OftSendParams,
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
use pinocchio_token::{self, state::TokenAccount};

pub struct PushLzBridgeAccounts<'info> {
    pub mint: &'info AccountInfo,
    pub vault: &'info AccountInfo,
    pub authority_token_account: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
    pub sysvar_instruction: &'info AccountInfo,
}

impl<'info> PushLzBridgeAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 7 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            mint: &account_infos[0],
            vault: &account_infos[1],
            authority_token_account: &account_infos[2],
            token_program: &account_infos[3],
            associated_token_program: &account_infos[4],
            system_program: &account_infos[5],
            sysvar_instruction: &account_infos[6],
        };
        let config = match config {
            IntegrationConfig::LzBridge(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if ctx.mint.key().ne(&config.mint) {
            msg! {"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.mint.key().ne(&config.mint) {
            msg! {"mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.token_program.key().ne(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"token_program: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx
            .associated_token_program
            .key()
            .ne(&pinocchio_associated_token_account::ID)
        {
            msg! {"associated_token_program: does not match config"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.system_program.key().ne(&pinocchio_system::ID) {
            msg! {"system_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        // TODO: Validate SysvarInstruction account
        // if ctx.sysvar_instruction.key().ne() {
        //     msg!{"sysvar_instruction: invalid address"};
        //     return Err(ProgramError::IncorrectProgramId);
        // }

        Ok(ctx)
    }
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

    ///////// CORE LOGIC ///////

    // TODO: Implement this once Pinocchio has transaction introspection capabilities

    // // 1. Check this instruction is at the first instruction of the transaction    <---- TBD if this step is necessary
    // let current_idx = load_current_index_checked(&inner_ctx.sysvar_instruction)?;
    // // if current_idx != 0 {
    // //     msg!{"current ixn is not first"}
    // //     return Err(ProgramError::InvalidInstructionData)
    // // };

    // // 2. Check the subsequent instruction is the OFT send instruction
    // let subsequent_ixn = load_instruction_at_checked(current_idx+1, &inner_ctx.sysvar_instruction)?;
    // if subsequent_ixn.program_id.ne(lz_program) {
    //     msg!{"subsequent ixn is to the wrong program"}
    //     return Err(ProgramError::IncorrectProgramId)
    // };

    // // 3. Decode the OFT send instruction data and assert the amount, destination_address and destination_eid
    // let send_params = OftSendParams::deserialize(&mut &subsequent_ixn.data[..]).unwrap();

    // // TODO: Calculate amount_ld from amount and OFTStore decimals
    // // TODO: Calculate min_amount_ld from amount and OFTStore decimals

    // if send_params.amount_ld != amount_ld {
    //     msg!{"subsequent ixn's amount does not match"}
    //     return Err(ProgramError::InvalidInstructionData)
    // };
    // if send_params.min_amount_ld != mint_amount_ld {
    //     msg!{"subsequent ixn's amount does not match"}
    //     return Err(ProgramError::InvalidInstructionData)
    // };
    // if send_params.dst_eid != config.destination_eid {
    //     msg!{"subsequent ixn's destination_eid does not match"}
    //     return Err(ProgramError::InvalidInstructionData)
    // };
    // if send_params.to.ne(config.destination_address) {
    //     msg!{"subsequent ixn's destination_address does not match"}
    //     return Err(ProgramError::InvalidInstructionData)
    // };

    // // 4. check the accounts of OFT Send instruction
    // // the signer of OFT Send instruction is the authority
    // if subsequent_ixn.accounts[0].pubkey.ne(outer_ctx.authority.key()) {
    //     msg!{"subsequent ixn's sender is not the authority"}
    //     return Err(ProgramError::InvalidInstructionData)
    // }

    // // check that the peer_config account matches that in the integration config
    // if subsequent_ixn.accounts[1].pubkey.ne(config.peer_config) {
    //     msg!{"subsequent ixn's peer_config does not match the integration config"}
    //     return Err(ProgramError::InvalidInstructionData)
    // }

    // // the token source account of OFT Send instruction is the token destination of this instruction
    // if subsequent_ixn.accounts[2].pubkey.ne(config.oft_store) {
    //     msg!{"subsequent ixn's oft_store does not match the integration config"}
    //     return Err(ProgramError::InvalidInstructionData)
    // }

    // // check the token mint of OFT Send instruction is the token mint of this instruction
    // if subsequent_ixn.accounts[5].pubkey.ne(inner_ctx.mint.key()) {
    //     msg!{"subsequent ixn's sender is not the authority"}
    //     return Err(ProgramError::InvalidInstructionData)
    // }

    // Creates the authority token_account, if necessary,
    //  or validates it
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
