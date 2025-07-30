use crate::{
    define_account_struct,
    instructions::UpdateOracleArgs,
    state::{nova_account::NovaAccount, Oracle},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

define_account_struct! {
    pub struct UpdateOracle<'info> {
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

    let oracle = &mut Oracle::load_and_check_mut(ctx.oracle)?;
    if oracle.authority.ne(ctx.authority.key()) {
        return Err(ProgramError::IncorrectAuthority);
    }

    // Update oracle_type and price_feed, if present.
    if let Some(feed_args) = args.feed_args {
        // Validate that new oracle_type matches price feed.
        Oracle::verify_oracle_type(feed_args.oracle_type, ctx.price_feed)?;
        oracle.feeds[0].oracle_type = feed_args.oracle_type;
        oracle.feeds[0].price_feed = *ctx.price_feed.key();
        oracle.feeds[0].invert_price = feed_args.invert_price;
        oracle.value = 0;
        oracle.precision = 0;
        oracle.last_update_slot = 0;
    }

    // Update authority, if present.
    let has_new_authority = ctx.new_authority.key().ne(program_id);
    if has_new_authority {
        oracle.authority = *ctx.new_authority.key();
    }
    oracle.save(ctx.oracle)?;

    Ok(())
}
