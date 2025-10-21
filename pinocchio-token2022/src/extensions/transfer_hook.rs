use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use super::get_extension_from_bytes;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]

pub struct TransferHook {
    /// Authority that can set the transfer hook program id
    pub authority: Pubkey,
    /// Program that authorizes the transfer
    pub program_id: Pubkey,
}

impl super::Extension for TransferHook {
    const TYPE: super::ExtensionType = super::ExtensionType::TransferHook;
    const LEN: usize = Self::LEN;
    const BASE_STATE: super::BaseState = super::BaseState::Mint;
}

impl TransferHook {
    /// The length of the `TransferHook` account data.
    pub const LEN: usize = core::mem::size_of::<TransferHook>();

    /// Return a `TransferHook` from the given account info.
    ///
    /// This method performs owner and length validation on `AccountInfo`, safe borrowing
    /// the account data.
    #[inline(always)]
    pub fn from_account_info_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&TransferHook, ProgramError> {
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
            .ok_or(ProgramError::InvalidAccountData)
    }
}
