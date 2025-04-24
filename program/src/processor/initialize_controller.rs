use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    enums::PermissionStatus, 
    events::{ControllerUpdateEvent, SvmAlmControllerEvent}, 
    instructions::InitializeControllerArgs,
    state::{Controller, Permission}
};

pub struct InitializeControllerAccounts<'info> {
    pub payer: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub controller: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> InitializeControllerAccounts<'info> {

    pub fn from_accounts(
        accounts: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if accounts.len() != 5 {
            return Err(ProgramError::NotEnoughAccountKeys)
        }
        let ctx = Self {
            payer: &accounts[0],
            authority: &accounts[1],
            controller: &accounts[2],
            permission: &accounts[3],
            system_program: &accounts[4],
        };
        if !ctx.payer.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.payer.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.controller.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.controller.is_owned_by(&pinocchio_system::id()) || !ctx.controller.data_is_empty() {
            return Err(ProgramError::InvalidAccountOwner)
        }
        if !ctx.permission.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.permission.is_owned_by(&pinocchio_system::id()) || !ctx.permission.data_is_empty() {
            return Err(ProgramError::InvalidAccountOwner)
        }
        if ctx.system_program.key().ne(&pinocchio_system::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(ctx)
    }
}


pub fn process_initialize_controller(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_controller");

    let ctx = InitializeControllerAccounts::from_accounts(
        accounts
    )?;

    // // Deserialize the args
    let args = InitializeControllerArgs::try_from_slice(
        instruction_data
    ).unwrap();

    // Initialize the controller data
    let controller = Controller::init_account(
        ctx.controller, 
        ctx.payer, 
        args.id,
        args.status
    )?;

    // Initialize the controller data
    Permission::init_account(
        ctx.permission, 
        ctx.payer, 
        *ctx.controller.key(),
        *ctx.authority.key(),
        PermissionStatus::Active,
        true, // Only can manage permissions to begin with
        false,
        false,
        false,
        false,
        false,
        false
    )?;    
    
    // Emit the event
    controller.emit_event(
        ctx.controller,
        SvmAlmControllerEvent::ControllerUpdate (
            ControllerUpdateEvent {
                controller: *ctx.controller.key(),
                authority: *ctx.authority.key(),
                old_state: None,
                new_state: Some(controller)
            }
        )
    )?;

    Ok(())
}

