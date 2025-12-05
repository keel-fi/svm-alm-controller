use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use super::get_extension_from_bytes;

/// State of the DefaultAccountState mint
/// Extension data source can be found here: https://github.com/solana-program/token-2022/blob/main/interface/src/extension/default_account_state/mod.rs
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultAccountStateConfig {
    /// Uninitialized 0
    /// Account is not yet initialized
    /// 
    /// Initialized 1
    /// Account is initialized; the account owner and/or delegate may perform
    /// permitted operations on this account
    /// 
    /// Frozen 2
    /// /// Account has been frozen by the mint freeze authority. Neither the
    /// account owner nor the delegate are able to perform operations on
    /// this account.
    pub state: PodAccountState
}

impl super::Extension for DefaultAccountStateConfig {
    const TYPE: super::ExtensionType = super::ExtensionType::DefaultAccountState;
    const LEN: usize = Self::LEN;
    const BASE_STATE: super::BaseState = super::BaseState::Mint;
}

impl DefaultAccountStateConfig {
    /// The length of the `PausableConfig` account data.
    pub const LEN: usize = core::mem::size_of::<DefaultAccountStateConfig>();

    /// Return a `DefaultAccountStateConfig` from the given account info.
    ///
    /// This method performs owner and length validation on `AccountInfo`, safe borrowing
    /// the account data.
    #[inline(always)]
    pub fn from_account_info_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&DefaultAccountStateConfig, ProgramError> {
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
            .ok_or(ProgramError::InvalidAccountData)
    }
}

type PodAccountState = u8;