extern crate alloc;

use alloc::vec::Vec;
use pinocchio::{
    account_info::{AccountInfo, Ref},
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    ProgramResult,
};
use pinocchio_token2022::extensions::ExtensionType;
use pinocchio_token_interface::get_all_extensions_for_mint;
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta, pubkey_data::PubkeyData, seeds::Seed, solana_pubkey::PUBKEY_BYTES,
    state::ExtraAccountMetaList,
};
use spl_type_length_value::state::TlvStateBorrowed;

use crate::{error::SvmAlmControllerErrors, state::discriminator};

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

const EXTRA_ACCOUNT_METAS_SEED: &[u8] = b"extra-account-metas";

/// Type for defining a required account in a validation account.
///
/// This can be any of the following:
///
/// * A standard `AccountMeta`
/// * A PDA (with seed configurations)
/// * A pubkey stored in some data (account or instruction data)
///
/// Can be used in TLV-encoded data.
// #[repr(C)]
// pub struct ExtraAccountMeta<'info> {
//     /// Discriminator to tell whether this represents a standard
//     /// `AccountMeta`, PDA, or pubkey data.
//     pub discriminator: u8,
//     /// This `address_config` field can either be the pubkey of the account,
//     /// the seeds used to derive the pubkey from provided inputs (PDA), or the
//     /// data used to derive the pubkey (account or instruction data).
//     pub address_config: &'info [u8; 32],
//     /// Whether the account should sign
//     pub is_signer: bool,
//     /// Whether the account should be writable
//     pub is_writable: bool,
// }

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
// impl<'info> ExtraAccountMeta<'info> {
//     const LEN: usize = 35;
// }

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
    let mut pda_seeds: Vec<&[u8]> = alloc::vec![];
    for config in seeds {
        match config {
            Seed::Uninitialized => (),
            Seed::Literal { bytes } => pda_seeds.push(bytes),
            Seed::InstructionData { index, length } => {
                let arg_start = *index as usize;
                let arg_end = arg_start + *length as usize;
                if arg_end > instruction_data.len() {
                    return Err(ProgramError::InvalidInstructionData);
                }
                pda_seeds.push(&instruction_data[arg_start..arg_end]);
            }
            Seed::AccountKey { index } => {
                let account_index = *index as usize;
                let address = get_account_key_data_fn(account_index)
                    .ok_or::<ProgramError>(ProgramError::NotEnoughAccountKeys)?
                    .0;
                pda_seeds.push(address.as_ref());
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

                // Create a new Ref with the sliced data
                let sliced_ref = Ref::map(account_data, |data| &data[arg_start..arg_end]);

                // Read the raw data from the sliced ref
                let slice = unsafe {
                    let ptr = sliced_ref.as_ptr();
                    core::slice::from_raw_parts(ptr, len)
                };

                pda_seeds.push(slice);
            }
        }
    }

    Ok(find_program_address(&pda_seeds, program_id).0)
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

// pub fn resolve<'a, F>(
//     data: &[u8],
//     instruction_data: &[u8],
//     program_id: &Pubkey,
//     get_account_key_data_fn: F,
// ) -> Result<TmpAccountMeta, ProgramError>
// where
//     F: Fn(usize) -> Option<(&'a Pubkey, Option<&'a [u8]>)>,
// {
//     let discriminator = data[0];
//     let address_config: [u8; 32] = data[1..33]
//         .try_into()
//         .map_err(|_| ProgramError::InvalidAccountData)?;
//     let is_signer = data[33] == 1;
//     let is_writable = data[34] == 1;
//     match self.discriminator {
//         0 => AccountMeta::try_from(self),
//         x if x == 1 || x >= U8_TOP_BIT => {
//             let program_id = if x == 1 {
//                 program_id
//             } else {
//                 get_account_key_data_fn(x.saturating_sub(U8_TOP_BIT) as usize)
//                     .ok_or::<ProgramError>(AccountResolutionError::AccountNotFound.into())?
//                     .0
//             };
//             let seeds = Seed::unpack_address_config(&self.address_config)?;
//             Ok(AccountMeta {
//                 pubkey: resolve_pda(
//                     &seeds,
//                     instruction_data,
//                     program_id,
//                     get_account_key_data_fn,
//                 )?,
//                 is_signer: self.is_signer.into(),
//                 is_writable: self.is_writable.into(),
//             })
//         }
//         2 => {
//             let key_data = PubkeyData::unpack(&self.address_config)?;
//             Ok(AccountMeta {
//                 pubkey: resolve_key_data(&key_data, instruction_data, get_account_key_data_fn)?,
//                 is_signer: self.is_signer.into(),
//                 is_writable: self.is_writable.into(),
//             })
//         }
//         _ => Err(ProgramError::InvalidAccountData),
//     }
// }

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

// pub fn resolve<'info>(
//     data: &'info [u8],
//     // TODO not sure if instruction data is same lifetime
//     instruction_data: &'info [u8],
//     // Accounts for the Transfer instruction
//     accounts: &'info [AccountInfo],
//     program_id: &Pubkey,
// ) -> Result<TmpAccountMeta, ProgramError> {
//     let discriminator = data[0];
//     let address_config: [u8; 32] = data[1..33]
//         .try_into()
//         .map_err(|_| ProgramError::InvalidAccountData)?;
//     let is_signer = data[33] == 1;
//     let is_writable = data[34] == 1;
//     match discriminator {
//         0 => Ok(TmpAccountMeta {
//             pubkey: address_config,
//             is_writable,
//             is_signer,
//         }),
//         x if x == 1 || x >= U8_TOP_BIT => {
//             let program_id = if x == 1 {
//                 program_id
//             } else {
//                 let account_index = x.saturating_sub(U8_TOP_BIT) as usize;
//                 accounts[account_index].key()
//             };

//             let seeds = Seed::unpack_address_config(&address_config)
//                 .map_err(|_| ProgramError::InvalidAccountData)?;

//             Ok(TmpAccountMeta {
//                 pubkey: resolve_pda(),
//                 is_writable: (),
//                 is_signer: (),
//             })
//         }
//         2 => {
//             let key_type = data[1];

//             match key_type {
//                 // InstructionData
//                 1 => {
//                     // The PubkeyData encoded the starting index for the Pubkey
//                     // within the instruction data.
//                     let start = data[2] as usize;
//                     let end = start + 32;
//                     Ok(TmpAccountMeta {
//                         pubkey: instruction_data[start..end]
//                             .try_into()
//                             .map_err(|_| ProgramError::InvalidInstructionData)?,
//                         is_signer,
//                         is_writable,
//                     })
//                 }
//                 // AccountData
//                 2 => {
//                     let account_index = data[2] as usize;
//                     let data_index = data[3] as usize;
//                     let end = data_index + 32;
//                     let account = &accounts[account_index];
//                     let account_data = account.try_borrow_data()?;

//                     Ok(TmpAccountMeta {
//                         pubkey: account_data[data_index..end]
//                             .try_into()
//                             .map_err(|_| ProgramError::InvalidAccountData)?,
//                         is_writable,
//                         is_signer,
//                     })
//                 }
//                 _ => Err(ProgramError::InvalidAccountData)?,
//             }
//         }
//         _ => Err(ProgramError::InvalidAccountData),
//     }
// }

// LIST of ExtraAccountMetaList Account structure
// 0..4 - length
// 4..N - [ExtraAccountMeta]

/// Get the state address PDA
pub fn get_extra_account_metas_address(mint: &Pubkey, program_id: &Pubkey) -> Pubkey {
    find_program_address(&[EXTRA_ACCOUNT_METAS_SEED, mint.as_ref()], program_id).0
}

// Attempting to keep similar interface to `add_extra_accounts_for_execute_cpi`. However,
// instead of adding the accounts to an existing instruction, we return the
// whole list of accounts.
pub fn create_transfer_instruction_with_transfer_hook(
    cpi_instruction_data: &[u8],
    cpi_account_infos: &mut Vec<AccountInfo>,
    transfer_hook_program_id: &Pubkey,
    source_info: AccountInfo,
    mint_info: AccountInfo,
    destination_info: AccountInfo,
    authority_info: AccountInfo,
    amount: u64,
    additional_accounts: &[AccountInfo],
) -> Result<(), ProgramError> {
    let extra_account_metas_address =
        get_extra_account_metas_address(mint_info.key(), transfer_hook_program_id);

    // Find the program info in the additional accounts
    let transfer_hook_program_info = additional_accounts
        .iter()
        .find(|&x| x.key().eq(transfer_hook_program_id))
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // TODO add the base accounts of the Transfer instruction here
    let mut ix_account_infos = Vec::new();
    let mut ix_account_metas = Vec::new();
    // Find the ExtraAccountMetas pubkey in the account list
    if let Some(extra_account_metas_info) = additional_accounts
        .iter()
        .find(|&x| x.key() == &extra_account_metas_address)
    {
        ix_account_infos.push(extra_account_metas_info);
        ix_account_metas.push(AccountMeta::readonly(&extra_account_metas_address));
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
                cpi_instruction_data,
                transfer_hook_program_id,
                |usize| {
                    ix_account_infos
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
            ix_account_metas.push(AccountMeta {
                pubkey: acct.key(),
                is_signer: tmp_meta.is_signer,
                is_writable: tmp_meta.is_writable,
            });
            ix_account_infos.push(acct);

            offset += EXTRA_ACCOUNT_META_LEN;
        }

        // Check AccountInfo for each in the AccountInfo list
        // for tmp_meta in account_metas {
        //     let acct = accounts
        //         .iter()
        //         .find(|acc_info| acc_info.key().eq(&tmp_meta.pubkey))
        //         .ok_or(ProgramError::NotEnoughAccountKeys)?;
        //     // De-escalate accounts by setting to the resolved value permissions
        //     ix_account_metas.push(AccountMeta {
        //         pubkey: acct.key(),
        //         is_signer: tmp_meta.is_signer,
        //         is_writable: tmp_meta.is_writable,
        //     });
        //     ix_account_infos.push(acct);
        // }
    }

    ix_account_infos.push(transfer_hook_program_info);
    ix_account_metas.push(AccountMeta::readonly(transfer_hook_program_id));
    // TODO NOTE: Does the invocation need to happen here to avoid returning
    // locally referenced data?

    Ok(())
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

// /// Helper to add accounts required for an `ExecuteInstruction` on-chain,
// /// looking through the additional account infos to add the proper accounts.
// ///
// /// Note this helper is designed to add the extra accounts that will be
// /// required for a CPI to a transfer hook program. However, the instruction
// /// being provided to this helper is for the program that will CPI to the
// /// transfer hook program. Because of this, we must resolve the extra accounts
// /// for the `ExecuteInstruction` CPI, then add those extra resolved accounts to
// /// the provided instruction.
// #[allow(clippy::too_many_arguments)]
// pub fn add_extra_accounts_for_execute_cpi<'a>(
//     cpi_instruction: &mut Instruction,
//     cpi_account_infos: &mut Vec<AccountInfo>,
//     program_id: &Pubkey,
//     source_info: AccountInfo,
//     mint_info: AccountInfo,
//     destination_info: AccountInfo,
//     authority_info: AccountInfo,
//     amount: u64,
//     additional_accounts: &[AccountInfo],
// ) -> ProgramResult {
//     let validate_state_pubkey = get_extra_account_metas_address(mint_info.key(), program_id);

//     let program_info = additional_accounts
//         .iter()
//         .find(|&x| x.key().eq(program_id))
//         .ok_or(TransferHookError::IncorrectAccount)?;

//     if let Some(validate_state_info) = additional_accounts
//         .iter()
//         .find(|&x| x.key().eq(&validate_state_pubkey))
//     {
//         // let mut execute_instruction = instruction::execute(
//         //     program_id,
//         //     source_info.key,
//         //     mint_info.key,
//         //     destination_info.key,
//         //     authority_info.key,
//         //     amount,
//         // );
//         execute_instruction
//             .accounts
//             .push(AccountMeta::new_readonly(validate_state_pubkey, false));
//         let mut execute_account_infos = vec![
//             source_info,
//             mint_info,
//             destination_info,
//             authority_info,
//             validate_state_info.clone(),
//         ];

//         ExtraAccountMetaList::add_to_cpi_instruction::<instruction::ExecuteInstruction>(
//             &mut execute_instruction,
//             &mut execute_account_infos,
//             &validate_state_info.try_borrow_data()?,
//             additional_accounts,
//         )?;

//         // Add only the extra accounts resolved from the validation state
//         cpi_instruction
//             .accounts
//             .extend_from_slice(&execute_instruction.accounts[5..]);
//         cpi_account_infos.extend_from_slice(&execute_account_infos[5..]);

//         // Add the validation state account
//         cpi_instruction
//             .accounts
//             .push(AccountMeta::new_readonly(validate_state_pubkey, false));
//         cpi_account_infos.push(validate_state_info.clone());
//     }

//     // Add the program id
//     cpi_instruction
//         .accounts
//         .push(AccountMeta::new_readonly(*program_id, false));
//     cpi_account_infos.push(program_info.clone());

//     Ok(())
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extra_account_meta_resolve() {
        let mut data = [1; ExtraAccountMeta::LEN];
        data[0] = 0;

        let expected_pubkey: Pubkey = [1u8; 32];
        let res = ExtraAccountMeta::resolve(&data, &[], &[]).unwrap();
        assert_eq!(res.pubkey, expected_pubkey);
        assert_eq!(res.is_signer, true);
        assert_eq!(res.is_writable, true);

        data[33] = 0;
        let res = ExtraAccountMeta::resolve(&data, &[], &[]).unwrap();
        assert_eq!(res.pubkey, expected_pubkey);
        assert_eq!(res.is_signer, false);
        assert_eq!(res.is_writable, true);
        data[34] = 0;
        let res = ExtraAccountMeta::resolve(&data, &[], &[]).unwrap();
        assert_eq!(res.pubkey, expected_pubkey);
        assert_eq!(res.is_signer, false);
        assert_eq!(res.is_writable, false);
    }
}
