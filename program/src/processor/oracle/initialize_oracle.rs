use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    events::{OracleUpdateEvent, SvmAlmControllerEvent},
    instructions::InitializeOracleArgs,
    state::{Controller, Oracle},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct InitializeOracle<'info> {
        payer: signer, mut;
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        price_feed;
        oracle: mut, empty, @owner(pinocchio_system::ID);
        system_program: @pubkey(pinocchio_system::ID);
    }
}

pub fn process_initialize_oracle(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_oracle");
    let ctx = InitializeOracle::from_accounts(accounts)?;
    let args = InitializeOracleArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Load and check controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;

    // Error when Controller is frozen
    if controller.is_frozen() {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    // Validate that oracle_type matches price feed.
    Oracle::verify_oracle_type(args.oracle_type, ctx.price_feed)?;

    let oracle = Oracle::init_account(
        ctx.oracle,
        ctx.authority,
        ctx.payer,
        ctx.controller.key(),
        &args.mint,
        &args.nonce,
        args.oracle_type,
        ctx.price_feed,
    )?;

    // Emit the Event to record the update
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::OracleUpdate(OracleUpdateEvent {
            controller: *ctx.controller.key(),
            oracle: *ctx.oracle.key(),
            authority: *ctx.authority.key(),
            old_state: None,
            new_state: Some(oracle),
        }),
    )?;

    Ok(())
}
