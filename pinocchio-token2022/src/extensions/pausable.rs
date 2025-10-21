use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use super::get_extension_from_bytes;

/// State of the pausable mint
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PausableConfig {
    /// Authority that can pause or resume activity on the mint
    pub authority: Pubkey,
    /// Whether minting / transferring / burning tokens is paused
    pub paused: u8,
}

impl super::Extension for PausableConfig {
    const TYPE: super::ExtensionType = super::ExtensionType::Pausable;
    const LEN: usize = Self::LEN;
    const BASE_STATE: super::BaseState = super::BaseState::Mint;
}

impl PausableConfig {
    /// The length of the `PausableConfig` account data.
    pub const LEN: usize = core::mem::size_of::<PausableConfig>();

    /// Return a `PausableConfig` from the given account info.
    ///
    /// This method performs owner and length validation on `AccountInfo`, safe borrowing
    /// the account data.
    #[inline(always)]
    pub fn from_account_info_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&PausableConfig, ProgramError> {
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
            .ok_or(ProgramError::InvalidAccountData)
    }
}
