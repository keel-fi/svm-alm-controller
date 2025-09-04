use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::instructions::{Instructions, INSTRUCTIONS_ID},
    ProgramResult,
};

use crate::{
    define_account_struct,
    enums::IntegrationState,
    error::SvmAlmControllerErrors,
    state::{keel_account::KeelAccount, Integration},
};

/// Permissionless instruction that must be called at the top
/// level of the Transaction (i.e. cannot be CPI'd) and
/// the last Instruction in the Transaction. This will reset
/// the in-flight flag allowing for another LZ Push instruction
/// to be sent.
define_account_struct! {
  pub struct ResetLzPushInFlight<'info> {
    controller;
    integration: mut;
    sysvar_instruction: @pubkey(INSTRUCTIONS_ID);
  }
}

/// Discriminator for ResetLzPushInFlight
pub const RESET_LZ_PUSH_IN_FLIGHT_DISC: u8 = 17;
/// Index of the Integration account in the `ResetLzPushInFlight` instruction.
pub const RESET_LZ_PUSH_INTEGRATION_INDEX: usize = 1;

/// Checks that ResetLzPushInFlight instruction is last in the Transaction
/// and has not been called via CPI.
pub fn validate_instruction(
    program_id: &Pubkey,
    sysvar_instruction: &AccountInfo,
) -> ProgramResult {
    // Get number of instructions in current transaction.
    let data = sysvar_instruction.try_borrow_data()?;
    if data.len() < 2 {
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }
    let ix_len = u16::from_le_bytes([data[0], data[1]]);

    let instructions = Instructions::try_from(sysvar_instruction)?;

    // Check that current ix is the last.
    let curr_index = instructions.load_current_index();
    if curr_index != ix_len - 1 {
        msg!("ResetLzPushInFlight must be last IX");
        return Err(SvmAlmControllerErrors::InvalidInstructionIndex.into());
    }

    // Load the top level instruction at the current index and
    // validate that it matches our program. If not, error as this
    // was called via a CPI.
    let curr_ix = instructions.load_instruction_at(curr_index as usize)?;
    if curr_ix.get_program_id().ne(program_id) {
        msg!("Cannot call ResetLzPushInFlight via CPI");
        return Err(ProgramError::IncorrectProgramId);
    }
    let curr_ix_data = curr_ix.get_instruction_data();
    if curr_ix_data[0] != RESET_LZ_PUSH_IN_FLIGHT_DISC {
        msg!("ResetLzPushInFlight invalid instruction");
        return Err(SvmAlmControllerErrors::InvalidInstructions.into());
    }

    Ok(())
}

pub fn process_reset_lz_push_in_flight(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("reset_lz_push_in_flight");
    let ctx = ResetLzPushInFlight::from_accounts(accounts)?;

    validate_instruction(program_id, ctx.sysvar_instruction)?;

    let mut integration = Integration::load_and_check(ctx.integration, ctx.controller.key())?;

    // Reset the LZ Push integration state to not be in-flight.
    match &mut integration.state {
        IntegrationState::LzBridge(state) => {
            state.push_in_flight = false;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    integration.save(ctx.integration)?;

    Ok(())
}
