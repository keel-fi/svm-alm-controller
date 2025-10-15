use crate::cpi_instruction;
use crate::{constants::anchor_discriminator, integrations::drift::constants::DRIFT_PROGRAM_ID};

cpi_instruction! {
    /// Initialize Drift UserStats account.
    /// This only needs to be called ONCE per Controller.
    /// NOTE: check for existence before invoking.
    pub struct InitializeUserStats<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "initialize_user_stats"),

        user_stats: Writable,
        state: Writable,
        authority: Signer,
        payer: Writable<Signer>,
        rent: Readonly,
        system_program: Readonly
    }
}

cpi_instruction! {
    /// Initialize Drift User account.
    /// This must be called per subaccount.
    /// NOTE: the Name on the User will simply be the Subaccount ID since
    /// we do not require a human readable Name.
    pub struct InitializeUser<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "initialize_user"),

        user: Writable,
        user_stats: Writable,
        state: Writable,
        authority: Signer,
        payer: Writable<Signer>,
        rent: Readonly,
        system_program: Readonly;

        sub_account_id: u16,
        name: [u8; 32]
    }
}

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    ProgramResult,
};
extern crate alloc;
use alloc::vec::Vec;

/// Manual implementation of PushDrift CPI instruction
pub struct PushDrift<'info> {
    pub state: &'info AccountInfo,
    pub user: &'info AccountInfo,
    pub user_stats: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub spot_market_vault: &'info AccountInfo,
    pub user_token_account: &'info AccountInfo,
    pub token_program: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
    pub market_index: u16,
    pub amount: u64,
    pub reduce_only: bool,
}

impl<'info> PushDrift<'info> {
    pub fn invoke(&self) -> ProgramResult {
        self.invoke_signed(&[])
    }

    pub fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {
        let base_accounts = [
            AccountMeta::new(self.state.key(), false, false), // Readonly
            AccountMeta::new(self.user.key(), true, false),   // Writable
            AccountMeta::new(self.user_stats.key(), true, false), // Writable
            AccountMeta::new(self.authority.key(), false, true), // Signer
            AccountMeta::new(self.spot_market_vault.key(), true, false), // Writable
            AccountMeta::new(self.user_token_account.key(), true, false), // Writable
            AccountMeta::new(self.token_program.key(), false, false), // Readonly
        ];

        // Create accounts vector with base accounts + remaining accounts
        let mut accounts = Vec::from(base_accounts);
        for account in self.remaining_accounts {
            accounts.push(AccountMeta::new(account.key(), false, false));
        }

        let mut data = anchor_discriminator("global", "deposit").to_vec();
        data.extend_from_slice(&self.market_index.to_le_bytes());
        data.extend_from_slice(&self.amount.to_le_bytes());
        data.extend_from_slice(&[self.reduce_only as u8]);

        let instruction = Instruction {
            program_id: &DRIFT_PROGRAM_ID,
            accounts: &accounts,
            data: &data,
        };

        // For now, only use the base accounts since we can't handle variable remaining accounts
        // with the current pinocchio invoke_signed signature
        let accounts_array= [
            self.state,
            self.user,
            self.user_stats,
            self.authority,
            self.spot_market_vault,
            self.user_token_account,
            self.token_program,
        ];
        let mut accounts_info = Vec::from(accounts_array);
        for account in self.remaining_accounts {
            accounts_info.push(account);
        }

        pinocchio::program::slice_invoke_signed(&instruction, accounts_info.as_slice(), signers)
    }
}
