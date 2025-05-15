use super::{
    discriminator::{AccountDiscriminators, Discriminator},
    nova_account::NovaAccount,
};
use crate::{
    constants::CONTROLLER_SEED,
    enums::ControllerStatus,
    events::SvmAlmControllerEvent,
    processor::shared::{create_pda_account, emit_cpi},
};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
};
use pinocchio_token::instructions::Transfer;
use shank::ShankAccount;
use solana_program::pubkey::Pubkey as SolanaPubkey;

#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct SwapPair {
    /// The token mint that is being used to make the swap
    pub input_mint: Pubkey,
    /// The token being swapped for
    pub output_mint: Pubkey,
}

impl Discriminator for SwapPair {
    const DISCRIMINATOR: u8 = AccountDiscriminators::SwapPair as u8;
}
