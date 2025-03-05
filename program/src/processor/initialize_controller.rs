use pinocchio::{account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::Pubkey, sysvars::{rent::Rent, Sysvar}, ProgramResult};
use crate::{
    constants::{CONTROLLER_SEED, PERMISSION_SEED}, enums::ControllerStatus, error::SvmAlmControllerErrors, processor::shared::{create_pda_account, verify_signer, verify_system_account, verify_system_program}, state::{
        discriminator::AccountSerialize,
        Controller, Permission
    } 
};
use solana_program::pubkey::Pubkey as SolanaPubkey;

pub fn process_initialize_controller(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_controller");

    let [payer_info, authority_info, controller_info, permission_info, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate: authority should have signed
    verify_signer(payer_info, false)?;
    // Validate: authority should have signed
    verify_signer(authority_info, false)?;
    // Validate: should be owned by system account, empty, and writable
    verify_system_account(controller_info, true)?;
    // Validate: should be owned by system account, empty, and writable
    verify_system_account(permission_info, true)?;
    // Validate: system program
    verify_system_program(system_program)?;

    // Deserialize the args
    let args = InitializeControllerArgs::try_from_bytes(instruction_data)?;

    // Create the Controller account and initialize it
    // NOTE: this could be optimized further by removing the `solana-program` dependency
    // and using `pubkey::checked_create_program_address` from Pinocchio to verify the
    // pubkey and associated bump (needed to be added as arg) is valid.
    let (controller_pda, controller_bump) = SolanaPubkey::find_program_address(
        &[CONTROLLER_SEED, args.id.to_be_bytes().as_ref()],
        &SolanaPubkey::from(*program_id),
    );
    if controller_info.key().ne(&controller_pda.to_bytes()) {
        return Err(SvmAlmControllerErrors::InvalidPda.into()); // PDA was invalid
    }
    // Space = Discriminator (1) + id (2) + bump (1) + status (1) = 5
    let rent = Rent::get()?;
    let controller_space = 5;
    let controller_id = args.id.to_be_bytes();
    let controller_bump_seed = [controller_bump];
    let controller_signer_seeds = [
        Seed::from(CONTROLLER_SEED),
        Seed::from(&controller_id),
        Seed::from(&controller_bump_seed),
    ];
    create_pda_account( payer_info, &rent, controller_space, program_id, controller_info, controller_signer_seeds )?;
    let controller = Controller {
        id: args.id,
        bump: controller_bump,
        status: args.status
    };
    let mut controller_data = controller_info.try_borrow_mut_data()?;
    controller_data.copy_from_slice(&controller.to_bytes());


    // Create the Permission account and initialize it
    // NOTE: this could be optimized further by removing the `solana-program` dependency
    // and using `pubkey::checked_create_program_address` from Pinocchio to verify the
    // pubkey and associated bump (needed to be added as arg) is valid.
    let controller_key = controller_info.key();
    let (permission_pda, permission_bump) = SolanaPubkey::find_program_address(
        &[PERMISSION_SEED,controller_key.as_ref(), authority_info.key().as_ref()],
        &SolanaPubkey::from(*program_id),
    );
    if permission_info.key().ne(&permission_pda.to_bytes()) {
        return Err(SvmAlmControllerErrors::InvalidPda.into()); // PDA was invalid
    }
    // Space = Discriminator (1) + controller (32) + authority (32) + status (1) + 12 bools  = 78
    let permission_space = 78;
    let permission_bump_seed = [permission_bump];
    let permission_signer_seeds = [
        Seed::from(PERMISSION_SEED),
        Seed::from(controller_key),
        Seed::from(authority_info.key()),
        Seed::from(&permission_bump_seed),
    ];
    create_pda_account( payer_info, &rent, permission_space, program_id, permission_info, permission_signer_seeds )?;
    let permission = Permission {
        controller: *controller_key,
        authority: *authority_info.key(),
        status: 1 // Active
    };
    let mut permission_data = permission_info.try_borrow_mut_data()?;
    permission_data.copy_from_slice(&permission.to_bytes());

    Ok(())
}


/// Instruction data for the `CreateCredential` instruction.
pub struct InitializeControllerArgs {
    id: u16,
    status: u8
}
impl InitializeControllerArgs {
    pub fn try_from_bytes(bytes: &[u8]) -> Result<InitializeControllerArgs, ProgramError> {
        if bytes.len() != 3 { return Err(ProgramError::InvalidInstructionData); }
        // Try interpret as a ControllerStatus or error
        ControllerStatus::try_from(bytes[2]).map_err(|_| ProgramError::InvalidArgument)?;
        Ok(InitializeControllerArgs {
            id: u16::from_le_bytes(bytes[0..2].try_into().unwrap()),
            status: bytes[2]
        })
    }
}