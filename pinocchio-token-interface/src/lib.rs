use core::ops::Deref;
use pinocchio::{
    account_info::{AccountInfo, Ref},
    program_error::ProgramError,
};

pub use pinocchio_token2022::instructions;

pub struct TokenAccount<'info>(Ref<'info, pinocchio_token2022::state::TokenAccount>);

impl<'info> TokenAccount<'info> {
    pub fn from_account_info(account_info: &'info AccountInfo) -> Result<Self, ProgramError> {
        if account_info.is_owned_by(&pinocchio_token2022::ID) {
            pinocchio_token2022::state::TokenAccount::from_account_info(account_info)
                .map(|t| TokenAccount(t))
                .map_err(|_| ProgramError::InvalidAccountData)
        } else if account_info.is_owned_by(&pinocchio_token::ID) {
            // Must have special handling for Tokekeg accounts to coerce into Token2022 Account.
            if account_info.data_len() != pinocchio_token::state::TokenAccount::LEN {
                return Err(ProgramError::InvalidAccountData);
            }
            // SAFETY: The Token and Token2022 TokenAccount structs are compatible in layout.
            Ok(TokenAccount(Ref::map(
                account_info.try_borrow_data()?,
                |data| unsafe {
                    pinocchio_token2022::state::TokenAccount::from_bytes_unchecked(data)
                },
            )))
        } else {
            Err(ProgramError::InvalidAccountData)
        }
    }
}

impl Deref for TokenAccount<'_> {
    type Target = pinocchio_token2022::state::TokenAccount;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Mint<'info>(Ref<'info, pinocchio_token2022::state::Mint>);

impl<'info> Mint<'info> {
    pub fn from_account_info(account_info: &'info AccountInfo) -> Result<Self, ProgramError> {
        if account_info.is_owned_by(&pinocchio_token2022::ID) {
            pinocchio_token2022::state::Mint::from_account_info(account_info)
                .map(|t| Mint(t))
                .map_err(|_| ProgramError::InvalidAccountData)
        } else if account_info.is_owned_by(&pinocchio_token::ID) {
            // Must have special handling for Tokenkeg accounts to coerce into Token2022 Account.
            if account_info.data_len() != pinocchio_token::state::Mint::LEN {
                return Err(ProgramError::InvalidAccountData);
            }
            // SAFETY: The Token and Token2022 Mint structs are compatible in layout.
            Ok(Mint(Ref::map(
                account_info.try_borrow_data()?,
                |data| unsafe { pinocchio_token2022::state::Mint::from_bytes_unchecked(data) },
            )))
        } else {
            Err(ProgramError::InvalidAccountData)
        }
    }
}

impl Deref for Mint<'_> {
    type Target = pinocchio_token2022::state::Mint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
