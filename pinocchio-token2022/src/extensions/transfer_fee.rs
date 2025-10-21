use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::extensions::get_extension_from_bytes;

use super::Extension;

/// Transfer fee configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferFee {
    /// First epoch where the transfer fee takes effect
    pub epoch: [u8; 8],
    /// Maximum fee assessed on transfers, expressed as an amount of tokens
    pub maximum_fee: [u8; 8],
    /// Amount of transfer collected as fees, expressed as basis points of the
    /// transfer amount, ie. increments of 0.01%
    pub transfer_fee_basis_points: [u8; 2],
}

/// State of the transfer fee configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferFeeConfig {
    /// Optional authority to set the fee
    pub transfer_fee_config_authority: Pubkey,
    /// Withdraw from mint instructions must be signed by this key
    pub withdraw_withheld_authority: Pubkey,
    /// Withheld transfer fee tokens that have been moved to the mint for
    /// withdrawal
    pub withheld_amount: [u8; 8],
    /// Older transfer fee, used if the current epoch < new_transfer_fee.epoch
    pub older_transfer_fee: TransferFee,
    /// Newer transfer fee, used if the current epoch >= new_transfer_fee.epoch
    pub newer_transfer_fee: TransferFee,
}

impl Extension for TransferFeeConfig {
    const TYPE: super::ExtensionType = super::ExtensionType::TransferFeeConfig;
    const LEN: usize = Self::LEN;
    const BASE_STATE: super::BaseState = super::BaseState::Mint;
}

impl TransferFeeConfig {
    /// The length of the `TransferFeeConfig` account data.
    pub const LEN: usize = core::mem::size_of::<TransferFeeConfig>();

    /// Return a `TransferFeeConfig` from the given Mint account info.
    ///
    /// This method performs owner and length validation on `AccountInfo`, safe borrowing
    /// the account data.
    #[inline(always)]
    pub fn from_account_info_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&TransferFeeConfig, ProgramError> {
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
            .ok_or(ProgramError::InvalidAccountData)
    }
}
