use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::state::Mint;
use crate::{
    error::SvmAlmControllerErrors, 
    events::{ReserveUpdateEvent, SvmAlmControllerEvent}, 
    instructions::InitializeReserveArgs, 
    state::{Controller, Permission, Reserve}
};


pub struct InitializeReserveAccounts<'info> {
    pub payer: &'info AccountInfo,
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub reserve: &'info AccountInfo,
    pub mint: &'info AccountInfo,
    pub vault: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> InitializeReserveAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() < 7 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            payer: &account_infos[0],
            controller: &account_infos[1],
            authority: &account_infos[2],
            permission: &account_infos[3],
            reserve: &account_infos[4],
            mint: &account_infos[5],
            vault: &account_infos[6],
            token_program: &account_infos[7],
            associated_token_program: &account_infos[8],
            system_program: &account_infos[9],
    };
        if !ctx.payer.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.payer.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.controller.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.permission.is_owned_by(&crate::ID) {
            msg!{"permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // New Integration AccountInfo must be mutable
        if !ctx.reserve.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.reserve.is_owned_by(&pinocchio_system::id()) {
            msg!{"reserve: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.reserve.data_is_empty() {
            msg!{"reserve: not empty"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.system_program.key().ne(&pinocchio_system::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }

        // TODO: Finish checks

        Ok(ctx)
    }
}



pub fn process_initialize_reserve(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_reserve");

    let ctx = InitializeReserveAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = InitializeReserveArgs::try_from_slice(
        instruction_data
    ).unwrap();
    
    // Load in controller state
    let controller = Controller::load_and_check(
        ctx.controller, 
    )?;

    // Load in the super permission account
    let permission = Permission::load_and_check(
        ctx.permission, 
        ctx.controller.key(), 
        ctx.authority.key()
    )?;
    // Check that super authority has permission and the permission is active
    if !permission.can_manage_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Validate the mint
    // Load in the mint account, validating it in the process
    Mint::from_account_info(ctx.mint).unwrap();

    // Invoke the CreateIdempotent ixn for the ATA
    // Will handle both the creation or the checking, if already created
    CreateIdempotent{
        funding_account: ctx.payer,
        account: ctx.vault,
        wallet: ctx.controller,
        mint: ctx.mint,
        system_program: ctx.system_program,
        token_program: ctx.token_program,
    }.invoke().unwrap();

    // Initialize the reserve account
    let mut reserve = Reserve::init_account(
        ctx.reserve, 
        ctx.payer, 
        *ctx.controller.key(),
        *ctx.mint.key(),
        *ctx.vault.key(),
        args.status,
        args.rate_limit_slope,
        args.rate_limit_max_outflow
    )?;

    // Emit the Event to record the update
    controller.emit_event(
        ctx.controller,
        SvmAlmControllerEvent::ReserveUpdate (
            ReserveUpdateEvent {
                controller: *ctx.controller.key(),
                reserve: *ctx.reserve.key(),
                authority: *ctx.authority.key(),
                old_state: None,
                new_state: Some(reserve)
            }
        )
    )?;
    
    // Call the initial sync balance
    reserve.sync_balance(
        ctx.vault, 
        ctx.controller, 
        &controller
    )?;

    // Save the account state
    reserve.save(ctx.reserve)?;

    Ok(())
}

