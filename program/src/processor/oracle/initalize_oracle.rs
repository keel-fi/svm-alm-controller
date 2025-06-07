use crate::{instructions::InitializeOracleArgs, state::Oracle};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub struct InitializeOracle<'info> {
    pub payer: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub price_feed: &'info AccountInfo,
    pub oracle: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> InitializeOracle<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            payer: &accounts[0],
            authority: &accounts[1],
            price_feed: &accounts[2],
            oracle: &accounts[3],
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
        if !ctx.oracle.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.oracle.data_is_empty() {
            msg! {"Oracle: not empty"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.oracle.is_owned_by(&pinocchio_system::id()) {
            msg! {"Oracle: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.system_program.key().ne(&pinocchio_system::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(ctx)
    }
}

pub fn process_initialize_oracle(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_oracle");
    let ctx = InitializeOracle::from_accounts(accounts)?;
    let args = InitializeOracleArgs::try_from_slice(instruction_data).unwrap();

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
