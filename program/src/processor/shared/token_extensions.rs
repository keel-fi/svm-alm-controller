use pinocchio::{account_info::AccountInfo, msg, pubkey::Pubkey, ProgramResult};
use pinocchio_token2022::extensions::{
    pausable::PausableConfig, transfer_hook::TransferHook, ExtensionType,
};
use pinocchio_token_interface::get_all_extensions_for_mint;

use crate::error::SvmAlmControllerErrors;

/// List of valid Mint extensions that can be used with
/// Integrations.
pub const VALID_MINT_EXTENSIONS: &[ExtensionType] = &[
    /* Purely UI, so no negative impact on Controller */
    ExtensionType::InterestBearingConfig,
    /* Purely UI, so no negative impact on Controller */
    ExtensionType::ScaledUiAmount,
    /* Tested for AtomicSwap and SplTokenExternal integrations */
    ExtensionType::TransferFeeConfig,
    ExtensionType::MintCloseAuthority,
    /*
        Could transfer/burn Controller tokens.
        Necessary for a lot of RWAs. Requires
        trusting of the issuer.
    */
    ExtensionType::PermanentDelegate,
    ExtensionType::ConfidentialTransferMint,
    ExtensionType::ConfidentialMintBurn,
    ExtensionType::MetadataPointer,
    ExtensionType::TokenMetadata,
    ExtensionType::GroupPointer,
    ExtensionType::TokenGroup,
    ExtensionType::GroupMemberPointer,
    ExtensionType::TokenGroupMember,
    /*
    Only allow TransferHooks if the hook program ID is null.
    Hook program being null is common for some RWA tokens.
    */
    ExtensionType::TransferHook,
    /*
    Could freeze within Controller. Requires trusting of the issuer.
        Only allowed if not paused.
    */
    ExtensionType::Pausable,
];

/// Validate the token extensions used by Token2022 token. If the mint
/// account data is larger than the base mint length, it means that there
/// are extensions present.
pub fn validate_mint_extensions(mint_acct: &AccountInfo) -> ProgramResult {
    if mint_acct.is_owned_by(&pinocchio_token2022::ID)
        && mint_acct.data_len() > pinocchio_token2022::state::Mint::BASE_LEN
    {
        let extension_types = get_all_extensions_for_mint(&mint_acct.try_borrow_data()?)?;
        for extension in extension_types {
            if extension == ExtensionType::Pausable {
                // Pausable is allowed, but we need to check that the mint is not paused
                let pausable_config = PausableConfig::from_account_info_unchecked(mint_acct)?;
                if pausable_config.paused > 0 {
                    msg!("Mint is paused");
                    return Err(SvmAlmControllerErrors::InvalidTokenMintExtension.into());
                }
            } else if extension == ExtensionType::TransferHook {
                // TransferHook is only allowed if the hook program ID is null
                let transfer_hook_config = TransferHook::from_account_info_unchecked(mint_acct)?;
                if transfer_hook_config.program_id.ne(&Pubkey::default()) {
                    msg!("Mint has invalid TransferHook program ID");
                    return Err(SvmAlmControllerErrors::InvalidTokenMintExtension.into());
                }
            } else if !VALID_MINT_EXTENSIONS.contains(&extension) {
                msg!("Mint has an invalid extension");
                return Err(SvmAlmControllerErrors::InvalidTokenMintExtension.into());
            }
        }
    }

    Ok(())
}
