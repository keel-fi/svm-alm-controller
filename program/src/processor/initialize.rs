use pinocchio::{account_info::AccountInfo, msg, pubkey::Pubkey, ProgramResult};

pub fn process_initialize(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("Initialize!");
    Ok(())
}
