use pinocchio::{account_info::AccountInfo, program_error::ProgramError};
use pinocchio_token2022::{
    extensions::{
        confidential_mint_burn::ConfidentialMintBurn,
        confidential_transfer::{
            ConfidentialTransferAccount, ConfidentialTransferFeeConfig, ConfidentialTransferMint,
        },
        confidential_transfer_fee::ConfidentialTransferFeeAmount,
        cpi_guard::CpiGuard,
        default_account_state::DefaultAccountState,
        group_member_pointer::GroupMemberPointer,
        group_pointer::GroupPointer,
        immutable_owner::ImmutableOwner,
        interest_bearing_mint::InterestBearingConfig,
        memo_transfer::MemoTransfer,
        metadata_pointer::MetadataPointer,
        mint_close_authority::MintCloseAuthority,
        non_transferable::{NonTransferable, NonTransferableAccount},
        pausable::{PausableAccount, PausableConfig},
        permanent_delegate::PermanentDelegate,
        scaled_ui_amount::ScaledUiAmountConfig,
        token_group::{TokenGroup, TokenGroupMember},
        transfer_fee::TransferFeeConfig,
        transfer_hook::{TransferHook, TransferHookAccount},
        BaseState, ExtensionType, EXTENSION_LENGTH_LEN, EXTENSION_TYPE_LEN,
    },
    state::TokenAccount,
};

use crate::get_all_extensions_for_mint;

/// Get the associated account type
pub fn get_account_type(extension_type: ExtensionType) -> BaseState {
    match extension_type {
        ExtensionType::Uninitialized => BaseState::Uninitialized,
        ExtensionType::TransferFeeConfig
        | ExtensionType::MintCloseAuthority
        | ExtensionType::ConfidentialTransferMint
        | ExtensionType::DefaultAccountState
        | ExtensionType::NonTransferable
        | ExtensionType::InterestBearingConfig
        | ExtensionType::PermanentDelegate
        | ExtensionType::TransferHook
        | ExtensionType::ConfidentialTransferFeeConfig
        | ExtensionType::MetadataPointer
        | ExtensionType::TokenMetadata
        | ExtensionType::GroupPointer
        | ExtensionType::TokenGroup
        | ExtensionType::GroupMemberPointer
        | ExtensionType::ConfidentialMintBurn
        | ExtensionType::TokenGroupMember
        | ExtensionType::ScaledUiAmount
        | ExtensionType::Pausable => BaseState::Mint,
        ExtensionType::ImmutableOwner
        | ExtensionType::TransferFeeAmount
        | ExtensionType::ConfidentialTransferAccount
        | ExtensionType::MemoTransfer
        | ExtensionType::NonTransferableAccount
        | ExtensionType::TransferHookAccount
        | ExtensionType::CpiGuard
        | ExtensionType::ConfidentialTransferFeeAmount
        | ExtensionType::PausableAccount => BaseState::TokenAccount,
    }
}

/// Util for calculate the space required for a
/// TokenAccount given a specific Mint.
pub fn get_account_data_size(
    new_extension_types: &[ExtensionType],
    mint_account: &AccountInfo,
) -> Result<usize, ProgramError> {
    if mint_account.is_owned_by(&pinocchio_token::ID) {
        // Short circuit when owned by SPL Token program
        return Ok(pinocchio_token2022::state::TokenAccount::BASE_LEN);
    }
    if new_extension_types
        .iter()
        .any(|&t| get_account_type(t) != BaseState::TokenAccount)
    {
        return Err(ProgramError::InvalidArgument);
    }

    let mint_extensions = get_all_extensions_for_mint(&mint_account.try_borrow_data()?)?;

    let mut account_extensions = get_required_init_account_extensions(&mint_extensions);
    // ExtensionType::try_calculate_account_len() dedupes types, so just a dumb
    // concatenation is fine here
    account_extensions.extend_from_slice(new_extension_types);

    try_calculate_account_len(&account_extensions)
}

/// Helper that tacks on the `AccountType` length, which gives the minimum for
/// any account with extensions
const BASE_ACCOUNT_AND_TYPE_LENGTH: usize = TokenAccount::BASE_LEN + 1;

// TODO make this more generic for Mint and TokenAccount.
// Only works for TokenAccount right now.
/// Get the required account data length for the given `ExtensionType`s
///
/// Fails if any of the extension types has a variable length
pub fn try_calculate_account_len(extension_types: &[ExtensionType]) -> Result<usize, ProgramError> {
    if extension_types.is_empty() {
        Ok(TokenAccount::BASE_LEN)
    } else {
        let extension_size = try_get_total_tlv_len(extension_types)?;
        // Equivalent of BASE_ACCOUNT_AND_TYPE_LENGTH
        let total_len = extension_size.saturating_add(BASE_ACCOUNT_AND_TYPE_LENGTH);
        Ok(adjust_len_for_multisig(total_len))
    }
}

/// Get the TLV length for a set of `ExtensionType`s
///
/// Fails if any of the extension types has a variable length
fn try_get_total_tlv_len(extension_types: &[ExtensionType]) -> Result<usize, ProgramError> {
    // dedupe extensions
    let mut extensions = vec![];
    for extension_type in extension_types {
        if !extensions.contains(&extension_type) {
            extensions.push(extension_type);
        }
    }
    extensions.iter().map(|e| try_get_tlv_len(e)).sum()
}

/// Get the TLV length for an `ExtensionType`
///
/// Fails if the extension type has a variable length
fn try_get_tlv_len(extension_type: &ExtensionType) -> Result<usize, ProgramError> {
    Ok(add_type_and_length_to_len(try_get_type_len(
        extension_type,
    )?))
}

/// Helper function to calculate exactly how many bytes a value will take up,
/// given the value's length
const fn add_type_and_length_to_len(value_len: usize) -> usize {
    value_len
        .saturating_add(EXTENSION_TYPE_LEN)
        .saturating_add(EXTENSION_LENGTH_LEN)
}

// TODO where to move this?
const MULTISIG_LEN: usize = 355;

/// Helper function to tack on the size of an extension bytes if an account with
/// extensions is exactly the size of a multisig
const fn adjust_len_for_multisig(account_len: usize) -> usize {
    if account_len == MULTISIG_LEN {
        account_len.saturating_add(core::mem::size_of::<ExtensionType>())
    } else {
        account_len
    }
}

pub fn get_required_init_account_extensions(
    mint_extension_types: &[ExtensionType],
) -> Vec<ExtensionType> {
    let mut account_extension_types = vec![];
    for extension_type in mint_extension_types {
        match extension_type {
            ExtensionType::TransferFeeConfig => {
                account_extension_types.push(ExtensionType::TransferFeeAmount);
            }
            ExtensionType::NonTransferable => {
                account_extension_types.push(ExtensionType::NonTransferableAccount);
                account_extension_types.push(ExtensionType::ImmutableOwner);
            }
            ExtensionType::TransferHook => {
                account_extension_types.push(ExtensionType::TransferHookAccount);
            }
            ExtensionType::Pausable => {
                account_extension_types.push(ExtensionType::PausableAccount);
            }
            _ => {}
        }
    }
    account_extension_types
}

/// Get the data length of the type associated with the enum
///
/// Fails if the extension type has a variable length
fn try_get_type_len(extension_type: &ExtensionType) -> Result<usize, ProgramError> {
    if !extension_type.sized() {
        return Err(ProgramError::InvalidArgument);
    }
    Ok(match extension_type {
        ExtensionType::Uninitialized => 0,
        ExtensionType::TransferFeeConfig => TransferFeeConfig::LEN,
        // TODO replace with value when available.
        // Len of 8 as type is:
        // pub struct TransferFeeAmount {
        //    pub withheld_amount: PodU64,
        // }
        ExtensionType::TransferFeeAmount => 8,
        ExtensionType::MintCloseAuthority => MintCloseAuthority::LEN,
        ExtensionType::ImmutableOwner => ImmutableOwner::LEN,
        ExtensionType::ConfidentialTransferMint => ConfidentialTransferMint::LEN,
        ExtensionType::ConfidentialTransferAccount => ConfidentialTransferAccount::LEN,
        ExtensionType::DefaultAccountState => DefaultAccountState::LEN,
        ExtensionType::MemoTransfer => MemoTransfer::LEN,
        ExtensionType::NonTransferable => NonTransferable::LEN,
        ExtensionType::InterestBearingConfig => InterestBearingConfig::LEN,
        ExtensionType::CpiGuard => CpiGuard::LEN,
        ExtensionType::PermanentDelegate => PermanentDelegate::LEN,
        ExtensionType::NonTransferableAccount => NonTransferableAccount::LEN,
        ExtensionType::TransferHook => TransferHook::LEN,
        ExtensionType::TransferHookAccount => TransferHookAccount::LEN,
        ExtensionType::ConfidentialTransferFeeConfig => ConfidentialTransferFeeConfig::LEN,
        ExtensionType::ConfidentialTransferFeeAmount => ConfidentialTransferFeeAmount::LEN,
        ExtensionType::MetadataPointer => MetadataPointer::LEN,
        ExtensionType::TokenMetadata => unreachable!(),
        ExtensionType::GroupPointer => GroupPointer::LEN,
        ExtensionType::TokenGroup => TokenGroup::LEN,
        ExtensionType::GroupMemberPointer => GroupMemberPointer::LEN,
        ExtensionType::TokenGroupMember => TokenGroupMember::LEN,
        ExtensionType::ConfidentialMintBurn => ConfidentialMintBurn::LEN,
        ExtensionType::ScaledUiAmount => ScaledUiAmountConfig::LEN,
        ExtensionType::Pausable => PausableConfig::LEN,
        ExtensionType::PausableAccount => PausableAccount::LEN,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_calculate_account_len() {
        let size = try_calculate_account_len(&[ExtensionType::ImmutableOwner]).unwrap();
        assert_eq!(
            size,
            BASE_ACCOUNT_AND_TYPE_LENGTH + EXTENSION_TYPE_LEN + EXTENSION_LENGTH_LEN
        );

        // Should handle multiple
        let size = try_calculate_account_len(&[
            ExtensionType::ImmutableOwner,
            ExtensionType::TransferFeeAmount,
        ])
        .unwrap();
        assert_eq!(
            size,
            BASE_ACCOUNT_AND_TYPE_LENGTH + 2 * (EXTENSION_TYPE_LEN + EXTENSION_LENGTH_LEN) + 8
        );

        // Should dedupe
        let size = try_calculate_account_len(&[
            ExtensionType::ImmutableOwner,
            ExtensionType::TransferFeeAmount,
            ExtensionType::TransferFeeAmount,
        ])
        .unwrap();
        assert_eq!(
            size,
            BASE_ACCOUNT_AND_TYPE_LENGTH + 2 * (EXTENSION_TYPE_LEN + EXTENSION_LENGTH_LEN) + 8
        );
    }
}
