use pinocchio::{
    account_info::AccountInfo,
    cpi::invoke_signed,
    instruction::{self, AccountMeta, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{write_bytes, UNINIT_BYTE};

use super::get_extension_from_bytes;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferHookAccount {
    /// Flag to indicate that the account is in the middle of a transfer
    pub transferring: u8,
}

impl super::Extension for TransferHookAccount {
    const TYPE: super::ExtensionType = super::ExtensionType::TransferHook;
    const LEN: usize = Self::LEN;
    const BASE_STATE: super::BaseState = super::BaseState::TokenAccount;
}

impl TransferHookAccount {
    /// The length of the `TransferHookAccount` account data.
    pub const LEN: usize = core::mem::size_of::<TransferHookAccount>();

    /// Return a `TransferHookAccount` from the given account info.
    ///
    /// This method performs owner and length validation on `AccountInfo`, safe borrowing
    /// the account data.
    #[inline(always)]
    pub fn from_account_info_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&TransferHookAccount, ProgramError> {
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
            .ok_or(ProgramError::InvalidAccountData)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]

pub struct TransferHook {
    /// Authority that can set the transfer hook program id
    pub authority: Pubkey,
    /// Program that authorizes the transfer
    pub program_id: Pubkey,
}

impl super::Extension for TransferHook {
    const TYPE: super::ExtensionType = super::ExtensionType::TransferHook;
    const LEN: usize = Self::LEN;
    const BASE_STATE: super::BaseState = super::BaseState::Mint;
}

impl TransferHook {
    /// The length of the `TransferHook` account data.
    pub const LEN: usize = core::mem::size_of::<TransferHook>();

    /// Return a `TransferHook` from the given account info.
    ///
    /// This method performs owner and length validation on `AccountInfo`, safe borrowing
    /// the account data.
    #[inline(always)]
    pub fn from_account_info_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&TransferHook, ProgramError> {
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        get_extension_from_bytes(unsafe { account_info.borrow_data_unchecked() })
            .ok_or(ProgramError::InvalidAccountData)
    }
}

// Instructions
pub struct Initialize<'a> {
    /// Mint of the transfer hook
    pub mint: &'a AccountInfo,
    /// The public key for the account that can update the transfer hook program id
    pub authority: Option<Pubkey>,
    /// The program id that authorizes the transfer
    pub program_id: Option<Pubkey>,
}

impl Initialize<'_> {
    #[inline(always)]
    pub fn invoke(&self) -> Result<(), ProgramError> {
        self.invoke_signed(&[])
    }

    pub fn invoke_signed(&self, signers: &[Signer]) -> Result<(), ProgramError> {
        // account metas
        let account_metas = [AccountMeta::writable(self.mint.key())];

        // Instruction data layout:
        // [0] : instruction discriminator (1 byte, u8)
        // [1] : extension instruction discriminator (1 byte, u8)
        // [2..34] : authority (32 bytes, Pubkey)
        // [34..66] : program_id (32 bytes, Pubkey)
        let mut instruction_data = [UNINIT_BYTE; 66];

        // Set discriminator as u8 at offset [0] & Set extension discriminator as u8 at offset [1]
        write_bytes(&mut instruction_data[0..2], &[36, 0]);
        // Set authority as u8 at offset [2..34]
        if let Some(authority) = self.authority {
            write_bytes(&mut instruction_data[2..34], &authority);
        } else {
            write_bytes(&mut instruction_data[2..34], &Pubkey::default());
        }
        // Set program_id as u8 at offset [34..66]
        if let Some(program_id) = self.program_id {
            write_bytes(&mut instruction_data[34..66], &program_id);
        } else {
            write_bytes(&mut instruction_data[34..66], &Pubkey::default());
        }
        let instruction = instruction::Instruction {
            program_id: &crate::ID,
            accounts: &account_metas,
            data: unsafe { core::slice::from_raw_parts(instruction_data.as_ptr() as _, 66) },
        };

        invoke_signed(&instruction, &[self.mint], signers)?;

        Ok(())
    }
}

pub struct Update<'a> {
    /// Mint of the transfer hook
    pub mint: &'a AccountInfo,
    /// The public key for the account that can update the transfer hook program id
    pub authority: &'a AccountInfo,
    /// The new program id that authorizes the transfer
    pub program_id: Option<Pubkey>,
}

impl Update<'_> {
    #[inline(always)]
    pub fn invoke(&self) -> Result<(), ProgramError> {
        self.invoke_signed(&[])
    }

    pub fn invoke_signed(&self, signers: &[Signer]) -> Result<(), ProgramError> {
        // account metas
        let account_metas = [
            AccountMeta::writable(self.mint.key()),
            AccountMeta::readonly_signer(self.authority.key()),
        ];

        // Instruction data layout:
        // [0] : instruction discriminator (1 byte, u8)
        // [1] : extension instruction discriminator (1 byte, u8)
        // [2..34] : authority (32 bytes, Pubkey)
        // [34..66] : program_id (32 bytes, Pubkey)
        let mut instruction_data = [UNINIT_BYTE; 66];

        // Set discriminator as u8 at offset [0] & Set extension discriminator as u8 at offset [1]
        write_bytes(&mut instruction_data[0..2], &[36, 1]);
        // Set program_id as u8 at offset [34..66]
        if let Some(program_id) = self.program_id {
            write_bytes(&mut instruction_data[34..66], &program_id);
        } else {
            write_bytes(&mut instruction_data[34..66], &Pubkey::default());
        }
        let instruction = instruction::Instruction {
            program_id: &crate::ID,
            accounts: &account_metas,
            data: unsafe { core::slice::from_raw_parts(instruction_data.as_ptr() as _, 66) },
        };

        invoke_signed(&instruction, &[self.mint], signers)?;

        Ok(())
    }
}

// #[allow(clippy::too_many_arguments)]
// pub fn add_extra_accounts_for_execute_cpi<'a>(
//     cpi_instruction: &mut Instruction,
//     cpi_account_infos: &mut Vec<AccountInfo<'a>>,
//     program_id: &Pubkey,
//     source_info: AccountInfo<'a>,
//     mint_info: AccountInfo<'a>,
//     destination_info: AccountInfo<'a>,
//     authority_info: AccountInfo<'a>,
//     amount: u64,
//     additional_accounts: &[AccountInfo<'a>],
// ) -> ProgramResult {
//     let validate_state_pubkey = get_extra_account_metas_address(mint_info.key, program_id);

//     let program_info = additional_accounts
//         .iter()
//         .find(|&x| x.key == program_id)
//         .ok_or(TransferHookError::IncorrectAccount)?;

//     if let Some(validate_state_info) = additional_accounts
//         .iter()
//         .find(|&x| *x.key == validate_state_pubkey)
//     {
//         let mut execute_instruction = instruction::execute(
//             program_id,
//             source_info.key,
//             mint_info.key,
//             destination_info.key,
//             authority_info.key,
//             amount,
//         );
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
