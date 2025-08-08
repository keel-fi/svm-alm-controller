use crate::{define_account_struct, instructions::InitializeOracleArgs, state::Oracle};
use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult};

define_account_struct! {
    pub struct InitializeOracle<'info> {
        payer: signer, mut;
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

    // Validate that oracle_type matches price feed.
    Oracle::verify_oracle_type(args.oracle_type, ctx.price_feed)?;

    Oracle::init_account(
        ctx.oracle,
        ctx.authority,
        ctx.payer,
        &args.nonce,
        args.oracle_type,
        ctx.price_feed,
        args.invert_price,
    )?;

    Ok(())
}
