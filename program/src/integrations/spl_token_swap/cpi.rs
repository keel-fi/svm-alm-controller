use borsh::{maybestd::vec::Vec, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct DepositSingleTokenTypeExactAmountInArgs {
    pub source_token_amount: u64,
    pub minimum_pool_token_amount: u64,
}

impl DepositSingleTokenTypeExactAmountInArgs {
    pub const DISCRIMINATOR: u8 = 4;
    pub const LEN: usize = 17;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized = Vec::with_capacity(1 + Self::LEN);
        serialized.push(Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized)
            .map_err(|_| ProgramError::InvalidInstructionData)?;
        Ok(serialized)
    }
}

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct WithdrawSingleTokenTypeExactAmountOutArgs {
    pub destination_token_amount: u64,
    pub maximum_pool_token_amount: u64,
}

impl WithdrawSingleTokenTypeExactAmountOutArgs {
    pub const DISCRIMINATOR: u8 = 5;
    pub const LEN: usize = 17;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized = Vec::with_capacity(1 + Self::LEN);
        serialized.push(Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized)
            .map_err(|_| ProgramError::InvalidInstructionData)?;
        Ok(serialized)
    }
}

pub fn deposit_single_token_type_exact_amount_in_cpi(
    amount: u64,
    signer: Signer,
    swap_program: Pubkey,
    swap: &AccountInfo,
    swap_authority: &AccountInfo,
    controller_authority: &AccountInfo,
    vault: &AccountInfo,
    swap_token_a: &AccountInfo,
    swap_token_b: &AccountInfo,
    lp_mint: &AccountInfo,
    lp_token_account: &AccountInfo,
    mint: &AccountInfo,
    mint_token_program: &AccountInfo,
    lp_mint_token_program: &AccountInfo,
) -> Result<(), ProgramError> {
    let args_vec = DepositSingleTokenTypeExactAmountInArgs {
        source_token_amount: amount,
        minimum_pool_token_amount: 0,
    }
    .to_vec()?;
    let data = args_vec.as_slice();
    invoke_signed(
        &Instruction {
            program_id: &swap_program,
            data: &data,
            accounts: &[
                AccountMeta::readonly(swap.key()),
                AccountMeta::readonly(swap_authority.key()),
                AccountMeta::readonly_signer(controller_authority.key()),
                AccountMeta::writable(vault.key()),
                AccountMeta::writable(swap_token_a.key()),
                AccountMeta::writable(swap_token_b.key()),
                AccountMeta::writable(lp_mint.key()),
                AccountMeta::writable(lp_token_account.key()),
                AccountMeta::readonly(mint.key()),
                AccountMeta::readonly(mint_token_program.key()),
                AccountMeta::readonly(lp_mint_token_program.key()),
            ],
        },
        &[
            swap,
            swap_authority,
            controller_authority,
            vault,
            swap_token_a,
            swap_token_b,
            lp_mint,
            lp_token_account,
            mint,
            mint_token_program,
            lp_mint_token_program,
        ],
        &[signer],
    )?;
    Ok(())
}

pub fn withdraw_single_token_type_exact_amount_out_cpi(
    amount: u64,
    signer: Signer,
    swap_program: Pubkey,
    swap: &AccountInfo,
    swap_authority: &AccountInfo,
    controller_authority: &AccountInfo,
    vault: &AccountInfo,
    swap_token_a: &AccountInfo,
    swap_token_b: &AccountInfo,
    lp_mint: &AccountInfo,
    lp_token_account: &AccountInfo,
    mint: &AccountInfo,
    mint_token_program: &AccountInfo,
    lp_mint_token_program: &AccountInfo,
    swap_fee_account: &AccountInfo,
) -> Result<(), ProgramError> {
    let args_vec = WithdrawSingleTokenTypeExactAmountOutArgs {
        destination_token_amount: amount,
        maximum_pool_token_amount: u64::MAX,
    }
    .to_vec()?;
    let data = args_vec.as_slice();
    invoke_signed(
        &Instruction {
            program_id: &swap_program,
            data: &data,
            accounts: &[
                AccountMeta::readonly(swap.key()),
                AccountMeta::readonly(swap_authority.key()),
                AccountMeta::readonly_signer(controller_authority.key()),
                AccountMeta::writable(lp_mint.key()),
                AccountMeta::writable(lp_token_account.key()),
                AccountMeta::writable(swap_token_a.key()),
                AccountMeta::writable(swap_token_b.key()),
                AccountMeta::writable(vault.key()),
                AccountMeta::writable(swap_fee_account.key()),
                AccountMeta::readonly(mint.key()),
                AccountMeta::readonly(lp_mint_token_program.key()),
                AccountMeta::readonly(mint_token_program.key()),
            ],
        },
        &[
            swap,
            swap_authority,
            controller_authority,
            lp_mint,
            lp_token_account,
            swap_token_a,
            swap_token_b,
            vault,
            swap_fee_account,
            mint,
            lp_mint_token_program,
            mint_token_program,
        ],
        &[signer],
    )?;
    Ok(())
}
