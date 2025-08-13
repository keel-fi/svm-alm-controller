use core::ops::Deref;
use pinocchio::{
    account_info::{AccountInfo, Ref},
    program_error::ProgramError,
};

use pinocchio_token2022::extensions::{
    ExtensionType, EXTENSIONS_PADDING, EXTENSION_LENGTH_LEN, EXTENSION_START_OFFSET,
    EXTENSION_TYPE_LEN,
};
pub use pinocchio_token2022::instructions;

pub mod utils;

pub use utils::*;

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

/// Iterate over all extension data and return the lists of extension types.
pub fn get_all_extensions_for_mint(
    acc_data_bytes: &[u8],
) -> Result<Vec<ExtensionType>, ProgramError> {
    let mut extension_types = Vec::new();
    let ext_bytes = &acc_data_bytes[pinocchio_token2022::state::Mint::BASE_LEN
        + EXTENSIONS_PADDING
        + EXTENSION_START_OFFSET..];
    let mut start = 0;
    let end = ext_bytes.len();
    while start < end {
        let ext_type_idx = start;
        let ext_len_idx = ext_type_idx + 2;

        let ext_type: [u8; 2] = ext_bytes[ext_type_idx..ext_type_idx + EXTENSION_TYPE_LEN]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;

        let ext_type =
            ExtensionType::from_bytes(ext_type).ok_or(ProgramError::InvalidAccountData)?;
        let ext_len: [u8; 2] = ext_bytes[ext_len_idx..ext_len_idx + EXTENSION_LENGTH_LEN]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;

        let ext_len = u16::from_le_bytes(ext_len);

        extension_types.push(ext_type);

        start = start + EXTENSION_TYPE_LEN + EXTENSION_LENGTH_LEN + ext_len as usize;
    }
    Ok(extension_types)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const TEST_MINT_WITH_EXTENSIONS_SLICE: &[u8] = &[
        1, 0, 0, 0, 221, 76, 72, 108, 144, 248, 182, 240, 7, 195, 4, 239, 36, 129, 248, 5, 24, 107,
        232, 253, 95, 82, 172, 209, 2, 92, 183, 155, 159, 103, 255, 33, 133, 204, 6, 44, 35, 140,
        0, 0, 6, 1, 1, 0, 0, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173,
        49, 41, 63, 207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        /*                  MintCloseAuthority Extension                                      */
        3, 0, 32, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41, 63,
        207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43,
        /*                  PermanentDelegate Extension                                      */
        12, 0, 32, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41,
        63, 207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43,
        /*                  TransferFeeConfig Extension                                      */
        1, 0, 108, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41,
        63, 207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43, 23, 133, 50, 97, 239, 106,
        184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41, 63, 207, 7, 207, 18, 10, 181, 185, 161,
        87, 6, 84, 141, 192, 43, 0, 0, 0, 0, 0, 0, 0, 0, 93, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 93, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        /*                  ConfidentialTransferMint Extension                                      */
        4, 0, 65, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41, 63,
        207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        /*                  ConfidentialTransferFeeConfig Extension                                      */
        16, 0, 129, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41,
        63, 207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43, 28, 55, 230, 67, 59, 115,
        4, 221, 130, 115, 122, 228, 13, 155, 139, 243, 196, 159, 91, 14, 108, 73, 168, 213, 51, 40,
        179, 229, 6, 144, 28, 87, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        /*                  TransferHook Extension                                      */
        14, 0, 64, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41,
        63, 207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        /*                  MetadataPointer Extension                                      */
        18, 0, 64, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41,
        63, 207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43, 23, 146, 72, 59, 108, 138,
        42, 135, 183, 71, 29, 129, 79, 149, 145, 249, 57, 92, 132, 10, 156, 227, 217, 244, 213,
        186, 125, 58, 75, 138, 116, 158,
        /*                  TokenMetadata Extension                                      */
        19, 0, 174, 0, 23, 133, 50, 97, 239, 106, 184, 83, 42, 103, 240, 83, 134, 90, 173, 49, 41,
        63, 207, 7, 207, 18, 10, 181, 185, 161, 87, 6, 84, 141, 192, 43, 23, 146, 72, 59, 108, 138,
        42, 135, 183, 71, 29, 129, 79, 149, 145, 249, 57, 92, 132, 10, 156, 227, 217, 244, 213,
        186, 125, 58, 75, 138, 116, 158, 10, 0, 0, 0, 80, 97, 121, 80, 97, 108, 32, 85, 83, 68, 5,
        0, 0, 0, 80, 89, 85, 83, 68, 79, 0, 0, 0, 104, 116, 116, 112, 115, 58, 47, 47, 116, 111,
        107, 101, 110, 45, 109, 101, 116, 97, 100, 97, 116, 97, 46, 112, 97, 120, 111, 115, 46, 99,
        111, 109, 47, 112, 121, 117, 115, 100, 95, 109, 101, 116, 97, 100, 97, 116, 97, 47, 112,
        114, 111, 100, 47, 115, 111, 108, 97, 110, 97, 47, 112, 121, 117, 115, 100, 95, 109, 101,
        116, 97, 100, 97, 116, 97, 46, 106, 115, 111, 110, 0, 0, 0, 0,
        /*                  GroupPointer Extension                                      */
        20, 0, 64, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 2, 2, 2, 2, 2,
        /*                  TokenGroup Extension                                      */
        21, 0, 80, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0,
    ];

    #[test]
    fn test_get_all_extensions_for_variable_pack() {
        let extension_types =
            get_all_extensions_for_mint(&TEST_MINT_WITH_EXTENSIONS_SLICE).unwrap();
        assert_eq!(
            extension_types,
            vec![
                ExtensionType::MintCloseAuthority,
                ExtensionType::PermanentDelegate,
                ExtensionType::TransferFeeConfig,
                ExtensionType::ConfidentialTransferMint,
                ExtensionType::ConfidentialTransferFeeConfig,
                ExtensionType::TransferHook,
                ExtensionType::MetadataPointer,
                ExtensionType::TokenMetadata,
                ExtensionType::GroupPointer,
                ExtensionType::TokenGroup,
            ]
        );
    }
}
