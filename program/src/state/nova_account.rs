use crate::acc_info_as_str;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_log::log;

pub trait NovaAccount {
    const LEN: usize;

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError>;

    fn verify_pda(&self, acc_info: &AccountInfo) -> Result<(), ProgramError> {
        let (controller_pda, _controller_bump) = self.derive_pda()?;
        if acc_info.key().ne(&controller_pda) {
            log!("PDA Mismatch for {}", acc_info_as_str!(acc_info));
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }
}
