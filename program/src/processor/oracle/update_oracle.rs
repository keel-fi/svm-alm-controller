use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    events::{OracleUpdateEvent, SvmAlmControllerEvent},
    instructions::UpdateOracleArgs,
    state::{keel_account::KeelAccount, Controller, Oracle},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use switchboard_on_demand::PRECISION;

define_account_struct! {
    pub struct UpdateOracle<'info> {
        controller: @owner(crate::ID);
        controller_authority: empty, @owner(pinocchio_system::ID);
        authority: signer;
        price_feed;
        oracle: mut;
        new_authority: opt_signer;
    }
}

pub fn process_update_oracle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("update_oracle");
    let ctx = UpdateOracle::from_accounts(accounts)?;
    let args = UpdateOracleArgs::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Load and check controller state
    let controller = Controller::load_and_check(ctx.controller, ctx.controller_authority.key())?;

    // Error when Controller is frozen
    if controller.is_frozen() {
        return Err(SvmAlmControllerErrors::ControllerFrozen.into());
    }

    let mut oracle = Oracle::load_and_check(
        ctx.oracle,
        Some(ctx.controller.key()),
        Some(ctx.authority.key()),
    )?;

    // Clone the old state for emitting event
    let old_state = oracle.clone();

    // Update oracle_type and price_feed, if present.
    if let Some(feed_args) = args.feed_args {
        // Validate that new oracle_type matches price feed.
        Oracle::verify_oracle_type(feed_args.oracle_type, ctx.price_feed)?;
        oracle.feeds[0].oracle_type = feed_args.oracle_type;
        oracle.feeds[0].price_feed = *ctx.price_feed.key();
        oracle.value = 0;
        match feed_args.oracle_type {
            0 => {
                // Switchboard on demand has fixed precision
                oracle.precision = PRECISION;
                Ok::<(), ProgramError>(())
            }
            _ => Err(SvmAlmControllerErrors::UnsupportedOracleType.into()),
        }?;
        oracle.last_update_slot = 0;
    }

    // Update authority, if present.
    let has_new_authority = ctx.new_authority.key().ne(program_id);
    if has_new_authority {
        oracle.authority = *ctx.new_authority.key();
    }

    // Emit the Event to record the update
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::OracleUpdate(OracleUpdateEvent {
            controller: *ctx.controller.key(),
            oracle: *ctx.oracle.key(),
            authority: *ctx.authority.key(),
            old_state: Some(old_state),
            new_state: Some(oracle),
        }),
    )?;

    oracle.save(ctx.oracle)?;

    Ok(())
}
