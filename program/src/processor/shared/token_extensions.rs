extern crate alloc;

use alloc::vec::Vec;
use pinocchio::{
    account_info::{AccountInfo, Ref},
    cpi::slice_invoke_signed,
    instruction::{AccountMeta, Instruction, Signer},
    msg,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    ProgramResult,
};
use pinocchio_token2022::extensions::ExtensionType;
use pinocchio_token_interface::get_all_extensions_for_mint;
use spl_tlv_account_resolution::{
    pubkey_data::PubkeyData, seeds::Seed, solana_pubkey::PUBKEY_BYTES,
};

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
    // WIP
    ExtensionType::TransferHook,
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

const EXTRA_ACCOUNT_METAS_SEED: &[u8] = b"extra-account-metas";

/// A struct similar to pinocchio::AccountMeta, but has
/// ownership of the Pubkey.
pub struct TmpAccountMeta {
    /// Public key of the account.
    pub pubkey: Pubkey,

    /// Indicates whether the account is writable or not.
    pub is_writable: bool,

    /// Indicates whether the account signed the instruction or not.
    pub is_signer: bool,
}

/// Helper used to know when the top bit is set, to interpret the
/// discriminator as an index rather than as a type
const U8_TOP_BIT: u8 = 1 << 7;
const EXTRA_ACCOUNT_META_LEN: usize = 35;

/// Resolve a program-derived address (PDA) from the instruction data
/// and the accounts that have already been resolved
fn resolve_pda<'a, F>(
    seeds: &[Seed],
    instruction_data: &[u8],
    program_id: &Pubkey,
    get_account_key_data_fn: F,
) -> Result<Pubkey, ProgramError>
where
    F: Fn(usize) -> Option<(&'a Pubkey, Option<Ref<'a, [u8]>>)>,
{
    let mut pda_seeds: Vec<Vec<u8>> = alloc::vec![];
    for config in seeds {
        match config {
            Seed::Uninitialized => (),
            Seed::Literal { bytes } => pda_seeds.push(bytes.clone()),
            Seed::InstructionData { index, length } => {
                let arg_start = *index as usize;
                let arg_end = arg_start + *length as usize;
                if arg_end > instruction_data.len() {
                    return Err(ProgramError::InvalidInstructionData);
                }
                pda_seeds.push(instruction_data[arg_start..arg_end].into());
            }
            Seed::AccountKey { index } => {
                let account_index = *index as usize;
                let address = get_account_key_data_fn(account_index)
                    .ok_or::<ProgramError>(ProgramError::NotEnoughAccountKeys)?
                    .0;
                pda_seeds.push(address.into());
            }
            Seed::AccountData {
                account_index,
                data_index,
                length,
            } => {
                let account_index = *account_index as usize;
                let account_data = get_account_key_data_fn(account_index)
                    .ok_or(ProgramError::NotEnoughAccountKeys)?
                    .1
                    .ok_or(ProgramError::NotEnoughAccountKeys)?;
                let arg_start = *data_index as usize;
                let len = *length as usize;
                let arg_end = arg_start + len;
                if account_data.len() < arg_end {
                    return Err(ProgramError::InvalidAccountData);
                }

                let data = account_data[arg_start..arg_end].to_vec();
                pda_seeds.push(data);
            }
        }
    }

    let seeds = pda_seeds
        .iter()
        .map(|seed| seed.as_slice())
        .collect::<Vec<&[u8]>>();
    Ok(find_program_address(&seeds, program_id).0)
}

/// Resolve a pubkey from a pubkey data configuration.
fn resolve_key_data<'a, F>(
    key_data: &PubkeyData,
    instruction_data: &[u8],
    get_account_key_data_fn: F,
) -> Result<Pubkey, ProgramError>
where
    F: Fn(usize) -> Option<(&'a Pubkey, Option<Ref<'a, [u8]>>)>,
{
    match key_data {
        PubkeyData::Uninitialized => Err(ProgramError::InvalidAccountData),
        PubkeyData::InstructionData { index } => {
            let key_start = *index as usize;
            let key_end = key_start + PUBKEY_BYTES;
            if key_end > instruction_data.len() {
                return Err(ProgramError::InvalidInstructionData);
            }
            Ok(instruction_data[key_start..key_end].try_into().unwrap())
        }
        PubkeyData::AccountData {
            account_index,
            data_index,
        } => {
            let account_index = *account_index as usize;
            let account_data = get_account_key_data_fn(account_index)
                .ok_or(ProgramError::NotEnoughAccountKeys)?
                .1
                .ok_or(ProgramError::NotEnoughAccountKeys)?;
            let arg_start = *data_index as usize;
            let arg_end = arg_start + PUBKEY_BYTES;
            if account_data.len() < arg_end {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(account_data[arg_start..arg_end].try_into().unwrap())
        }
    }
}

/// Resolve an `ExtraAccountMeta` into an `TmpAccountMeta`, potentially
/// resolving a program-derived address (PDA) if necessary
pub fn resolve<'info, F>(
    data: &'info [u8],
    instruction_data: &'info [u8],
    program_id: &Pubkey,
    get_account_key_data_fn: F,
) -> Result<TmpAccountMeta, ProgramError>
where
    F: Fn(usize) -> Option<(&'info Pubkey, Option<Ref<'info, [u8]>>)>,
{
    let discriminator = data[0];
    let address_config: [u8; 32] = data[1..33]
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let is_signer = data[33] == 1;
    let is_writable = data[34] == 1;
    match discriminator {
        0 => Ok(TmpAccountMeta {
            pubkey: address_config,
            is_writable,
            is_signer,
        }),
        x if x == 1 || x >= U8_TOP_BIT => {
            let program_id = if x == 1 {
                program_id
            } else {
                get_account_key_data_fn(x.saturating_sub(U8_TOP_BIT) as usize)
                    .ok_or(ProgramError::NotEnoughAccountKeys)?
                    .0
            };

            let seeds = Seed::unpack_address_config(&address_config)
                .map_err(|_| ProgramError::InvalidAccountData)?;

            Ok(TmpAccountMeta {
                pubkey: resolve_pda(
                    &seeds,
                    instruction_data,
                    program_id,
                    get_account_key_data_fn,
                )?,
                is_writable,
                is_signer,
            })
        }
        2 => {
            let key_data = PubkeyData::unpack(&address_config)
                .map_err(|_| ProgramError::InvalidAccountData)?;
            Ok(TmpAccountMeta {
                pubkey: resolve_key_data(&key_data, instruction_data, get_account_key_data_fn)?,
                is_writable,
                is_signer,
            })
        }
        _ => Err(ProgramError::InvalidAccountData),
    }
}

/// Get the state address PDA
pub fn get_extra_account_metas_address(mint: &Pubkey, program_id: &Pubkey) -> Pubkey {
    find_program_address(&[EXTRA_ACCOUNT_METAS_SEED, mint.as_ref()], program_id).0
}

// Attempting to keep similar interface to `add_extra_accounts_for_execute_cpi`. However,
// instead of adding the accounts to an existing instruction, we return the
// whole list of accounts.
//
pub fn invoke_transfer_checked_with_transfer_hook(
    transfer_hook_program_id: &Pubkey,
    source_info: &AccountInfo,
    mint_info: &AccountInfo,
    destination_info: &AccountInfo,
    authority_info: &AccountInfo,
    amount: u64,
    decimals: u8,
    additional_accounts: &[AccountInfo],
    signers_seeds: &[Signer],
) -> Result<(), ProgramError> {
    let extra_account_metas_address =
        get_extra_account_metas_address(mint_info.key(), transfer_hook_program_id);

    // Find the program info in the additional accounts
    let transfer_hook_program_info = additional_accounts
        .iter()
        .find(|&x| x.key().eq(transfer_hook_program_id))
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // TransferChecked Instruction data layout:
    // -  [0]: instruction discriminator (1 byte, u8)
    // -  [1..9]: amount (8 bytes, u64)
    // -  [9]: decimals (1 byte, u8)
    let mut transfer_checked_ix_data = [0u8; 10];
    transfer_checked_ix_data[0] = 12;
    transfer_checked_ix_data[1..9].copy_from_slice(&amount.to_le_bytes());
    transfer_checked_ix_data[9] = decimals;
    // Accounts for the TransferChecked instruction
    let mut transfer_checked_ix_account_infos: Vec<&AccountInfo> =
        alloc::vec![&source_info, &mint_info, &destination_info, &authority_info];
    let mut transfer_checked_ix_account_metas = alloc::vec![
        AccountMeta::writable(source_info.key()),
        AccountMeta::readonly(mint_info.key()),
        AccountMeta::writable(destination_info.key()),
        AccountMeta::readonly_signer(authority_info.key()),
    ];
    // Find the ExtraAccountMetas pubkey in the account list
    if let Some(extra_account_metas_info) = additional_accounts
        .iter()
        .find(|&x| x.key() == &extra_account_metas_address)
    {
        transfer_checked_ix_account_infos.push(extra_account_metas_info);
        transfer_checked_ix_account_metas.push(AccountMeta::readonly(&extra_account_metas_address));

        // ExtraAccountMetaList Account structure
        // 0..4 - length
        // 4..N - [ExtraAccountMeta]
        let extra_account_metas_data = extra_account_metas_info.try_borrow_data()?;
        let length = u32::from_le_bytes(
            extra_account_metas_data[0..4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let mut offset = 4;
        // Resolve all ExtraAccountMeta into list
        for _index in 0..length {
            let extra_meta_data =
                &extra_account_metas_data[offset..offset + EXTRA_ACCOUNT_META_LEN];

            let tmp_meta = resolve(
                extra_meta_data,
                &transfer_checked_ix_data,
                transfer_hook_program_id,
                |usize| {
                    transfer_checked_ix_account_infos
                        .get(usize)
                        .map(|acc_info| (acc_info.key(), acc_info.try_borrow_data().ok()))
                },
            )?;

            // Check AccountInfo exists in the account list
            let acct = additional_accounts
                .iter()
                .find(|acc_info| acc_info.key().eq(&tmp_meta.pubkey))
                .ok_or(ProgramError::NotEnoughAccountKeys)?;

            // De-escalate accounts by setting to the resolved value permissions
            transfer_checked_ix_account_metas.push(AccountMeta {
                pubkey: acct.key(),
                is_signer: tmp_meta.is_signer,
                is_writable: tmp_meta.is_writable,
            });
            transfer_checked_ix_account_infos.push(acct);

            offset += EXTRA_ACCOUNT_META_LEN;
        }
    }

    transfer_checked_ix_account_infos.push(transfer_hook_program_info);
    transfer_checked_ix_account_metas.push(AccountMeta::readonly(transfer_hook_program_id));

    // TODO this should be TransferChecked instruction to a specified TokenProgram
    let transfer_checked_with_transfer_hook = Instruction {
        program_id: transfer_hook_program_id,
        accounts: &transfer_checked_ix_account_metas,
        data: &transfer_checked_ix_data,
    };

    slice_invoke_signed(&transfer_checked_with_transfer_hook, &transfer_checked_ix_account_infos, signers_seeds)
}

pub enum TransferHookError {
    /// Incorrect account provided
    IncorrectAccount = 2_110_272_652,
    /// Mint has no mint authority
    MintHasNoMintAuthority,
    /// Incorrect mint authority has signed the instruction
    IncorrectMintAuthority,
    /// Program called outside of a token transfer
    ProgramCalledOutsideOfTransfer,
}

impl From<TransferHookError> for ProgramError {
    fn from(e: TransferHookError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

#[cfg(test)]
mod tests {
    use spl_tlv_account_resolution::account::ExtraAccountMeta;

    use super::*;

    #[test]
    fn test_extra_account_meta_resolve() {
        let extra_account_meta = ExtraAccountMeta {
            discriminator: 0,
            address_config: [1; 32],
            is_signer: true.into(),
            is_writable: true.into(),
        };

        let program_id = [0u8; 32];
        let accounts: Vec<AccountInfo> = alloc::vec![];
        let get_account_data = |usize| {
            accounts
                .get(usize)
                .map(|acc_info: &AccountInfo| (acc_info.key(), acc_info.try_borrow_data().ok()))
        };

        let bytes = bytemuck::bytes_of(&extra_account_meta);
        let expected_pubkey: Pubkey = [1u8; 32];
        let res = resolve(&bytes, &[], &program_id, get_account_data).unwrap();
        assert_eq!(res.pubkey, expected_pubkey);
        assert_eq!(res.is_signer, true);
        assert_eq!(res.is_writable, true);
    }
}
