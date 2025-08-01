use pinocchio::{account_info::AccountInfo, msg, ProgramResult};
use pinocchio_token2022::extensions::ExtensionType;
use pinocchio_token_interface::get_all_extensions_for_mint;

use crate::error::SvmAlmControllerErrors;

/// List of valid Mint extensions that can be used with
/// Integrations.
pub const VALID_MINT_EXTENSIONS: &[ExtensionType] = &[
    /* UNTESTED Purely UI, so no negative impact on Controller */
    ExtensionType::InterestBearingConfig,
    /* UNTESTED Purely UI, so no negative impact on Controller */
    ExtensionType::ScaledUiAmount,
    /* Tested for AtomicSwap and SplTokenExternal integrations */
    ExtensionType::TransferFeeConfig,
    /* UNTESTED */
    ExtensionType::MintCloseAuthority,
    /*
        UNTESTED Could transfer/burn Controller tokens.
        Necessary for a lot of RWAs. Requires
        trusting of the issuer.
    */
    ExtensionType::PermanentDelegate,
    /* UNTESTED Could freeze within Controller. Requires trusting of the issuer. */
    ExtensionType::Pausable,
    // TODO need to handle remaining accounts to enable
    // ExtensionType::TransferHook,
    /* UNTESTED */
    ExtensionType::MemoTransfer,
    /* UNTESTED */
    ExtensionType::ConfidentialMintBurn,
    /* UNTESTED */
    ExtensionType::MetadataPointer,
    /* UNTESTED */
    ExtensionType::TokenMetadata,
    /* UNTESTED */
    ExtensionType::GroupPointer,
    /* UNTESTED */
    ExtensionType::TokenGroup,
    /* UNTESTED */
    ExtensionType::GroupMemberPointer,
    /* UNTESTED */
    ExtensionType::TokenGroupMember,
];

/// Validate the token extensions used by Token2022 token. If the mint
/// account data is larger than the base mint length, it means that there
/// are extensions present.
pub fn validate_mint_extensions(mint_acct: &AccountInfo) -> ProgramResult {
    if mint_acct.is_owned_by(&pinocchio_token2022::ID)
        && mint_acct.data_len() > pinocchio_token2022::state::Mint::BASE_LEN
    {
        let extension_types = get_all_extensions_for_mint(&mint_acct.try_borrow_data()?)?;
        if extension_types
            .iter()
            .any(|ext| !VALID_MINT_EXTENSIONS.contains(ext))
        {
            msg!("Mint has an invalid extension");
            return Err(SvmAlmControllerErrors::InvalidTokenMintExtension.into());
        }
    }

    Ok(())
}
