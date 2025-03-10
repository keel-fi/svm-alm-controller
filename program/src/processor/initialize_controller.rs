use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use crate::{
    constants::CONTROLLER_SEED, enums::PermissionStatus, events::{ControllerUpdateEvent, SvmAlmControllerEvent}, instructions::InitializeControllerArgs, processor::shared::emit_cpi, state::{Controller, Permission}
};

pub struct InitializeControllerAccounts<'info> {
    pub payer_info: &'info AccountInfo,
    pub authority_info: &'info AccountInfo,
    pub controller_info: &'info AccountInfo,
    pub permission_info: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> InitializeControllerAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 5 {
            return Err(ProgramError::NotEnoughAccountKeys)
        }
        let ctx = Self {
            payer_info: &account_infos[0],
            authority_info: &account_infos[1],
            controller_info: &account_infos[2],
            permission_info: &account_infos[3],
            system_program: &account_infos[4],
        };
        if !ctx.payer_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.payer_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.authority_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.controller_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.controller_info.owner().ne(&pinocchio_system::id()) || !ctx.controller_info.data_is_empty() {
            return Err(ProgramError::InvalidAccountOwner)
        }
        if !ctx.permission_info.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.permission_info.owner().ne(&pinocchio_system::id()) || !ctx.permission_info.data_is_empty() {
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

    let ctx = InitializeControllerAccounts::from_accounts(accounts)?;

    // // Deserialize the args
    let args = InitializeControllerArgs::try_from_slice(
        instruction_data
    ).unwrap();
    

    // Initialize the controller data
    let controller = Controller::init_account(
        ctx.controller_info, 
        ctx.payer_info, 
        args.id,
        args.status
    )?;

    // Initialize the controller data
    Permission::init_account(
        ctx.permission_info, 
        ctx.payer_info, 
        *ctx.controller_info.key(),
        *ctx.authority_info.key(),
        PermissionStatus::Active,
        true, // Only can manage permissions to begin with
        false,
        false,
        false,
        false,
        false,
        false
    )?;    
    // Emit the Event
    emit_cpi(
        ctx.controller_info,
        [
            Seed::from(CONTROLLER_SEED),
            Seed::from(&controller.id.to_le_bytes()),
            Seed::from(&[controller.bump])
        ],
        SvmAlmControllerEvent::ControllerUpdate (
            ControllerUpdateEvent {
                controller: *ctx.controller_info.key(),
                authority: *ctx.authority_info.key(),
                old_state: None,
                new_state: Some(controller)
            }
        )
    )?;

    Ok(())
}

