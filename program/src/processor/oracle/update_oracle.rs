use crate::{
    instructions::UpdateOracleArgs,
    state::{nova_account::NovaAccount, Oracle},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub struct UpdateOracle<'info> {
    pub authority: &'info AccountInfo,
    pub price_feed: &'info AccountInfo,
    pub oracle: &'info AccountInfo,
    pub new_authority: &'info AccountInfo,
}

impl<'info> UpdateOracle<'info> {
    pub fn from_accounts(
        program_id: &Pubkey,
        accounts: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if accounts.len() < 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            authority: &accounts[0],
            price_feed: &accounts[1],
            oracle: &accounts[2],
            new_authority: &accounts[3],
        };
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.oracle.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }

        // Optional account defaults to program_id if not present.
        let has_new_authority = ctx.new_authority.key().ne(program_id);
        if has_new_authority && !ctx.new_authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(ctx)
    }
}

pub fn process_update_oracle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("update_oracle");
    let ctx = UpdateOracle::from_accounts(program_id, accounts)?;
    let args = UpdateOracleArgs::try_from_slice(instruction_data).unwrap();

    let oracle = &mut Oracle::load_and_check_mut(ctx.oracle)?;
    if oracle.authority.ne(ctx.authority.key()) {
        return Err(ProgramError::IncorrectAuthority);
    }

    // Update oracle_type and price_feed, if present.
    if let Some(oracle_type) = args.oracle_type {
        // Validate that new oracle_type matches price feed.
        Oracle::verify_oracle_type(oracle_type, ctx.price_feed)?;
        oracle.oracle_type = oracle_type;
        oracle.price_feed = *ctx.price_feed.key();
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
