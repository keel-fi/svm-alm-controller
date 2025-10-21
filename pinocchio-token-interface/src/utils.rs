use pinocchio_token2022::extensions::{BaseState, ExtensionType};

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
