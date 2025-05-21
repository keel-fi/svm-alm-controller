use crate::{instructions::InitializeOracleArgs, state::Oracle};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub struct InitializeOracle<'info> {
    pub payer: &'info AccountInfo,
    pub price_feed: &'info AccountInfo,
    pub oracle: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> InitializeOracle<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            payer: &accounts[0],
            price_feed: &accounts[1],
            oracle: &accounts[2],
            system_program: &accounts[3],
        };
        if !ctx.payer.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.payer.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        // TODO: Check price feed here?
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
    // TODO: Check signer?
    let ctx = InitializeOracle::from_accounts(accounts)?;
    let args = InitializeOracleArgs::try_from_slice(instruction_data).unwrap();

    Oracle::init_account(ctx.oracle, ctx.payer, ctx.price_feed, args.oracle_type)?;

    Ok(())
}
