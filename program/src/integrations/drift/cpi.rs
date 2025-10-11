use borsh::maybestd::string::ToString;
use pinocchio::{
    account_info::AccountInfo,
    cpi::invoke_signed,
    instruction::{AccountMeta, Instruction, Signer},
    ProgramResult,
};
use pinocchio_log::log;

use crate::{constants::anchor_discriminator, integrations::drift::constants::DRIFT_PROGRAM_ID};

pub struct InitializeUserStats<'info> {
    pub user_stats: &'info AccountInfo,
    pub state: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub payer: &'info AccountInfo,
    pub rent: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> InitializeUserStats<'info> {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "initialize_user_stats");

    /// Initialize Drift UserStats account.
    /// This only needs to be called ONCE per Controller.
    /// NOTE: check for existence before invoking.
    pub fn invoke_signed(&self, signers_seeds: Signer) -> ProgramResult {
        let account_infos = [
            self.user_stats,
            self.state,
            self.authority,
            self.payer,
            self.rent,
            self.system_program,
        ];
        let accounts = [
            AccountMeta::new(self.user_stats.key(), true, false),
            AccountMeta::new(self.state.key(), true, false),
            AccountMeta::new(self.authority.key(), false, true),
            AccountMeta::new(self.payer.key(), true, true),
            AccountMeta::new(self.rent.key(), false, false),
            AccountMeta::new(self.system_program.key(), false, false),
        ];
        let ix = Instruction {
            program_id: &DRIFT_PROGRAM_ID,
            accounts: &accounts,
            data: &Self::DISCRIMINATOR,
        };
        invoke_signed(&ix, &account_infos, &[signers_seeds])
    }
}

pub struct InitializeUser<'info> {
    pub user: &'info AccountInfo,
    pub user_stats: &'info AccountInfo,
    pub state: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub payer: &'info AccountInfo,
    pub rent: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}

impl<'info> InitializeUser<'info> {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "initialize_user");

    /// Initialize Drift User account.
    /// This must be called per subaccount.
    /// NOTE: the Name on the User will simply be the Subaccount ID since
    /// we do not require a human readable Name.
    pub fn invoke_signed(&self, sub_account_id: u16, signers_seeds: Signer) -> ProgramResult {
        // 8 - disc
        // 2 - subaccount id
        // 32 - Name
        let mut data = [0u8; 42];
        data[..8].copy_from_slice(&Self::DISCRIMINATOR);
        data[8..10].copy_from_slice(&sub_account_id.to_le_bytes());
        // let sub_account_string = sub_account_id.to_string();
        // data[10..sub_account_string.len()].copy_from_slice(&sub_account_string.as_bytes());

        let account_infos = [
            self.user,
            self.user_stats,
            self.state,
            self.authority,
            self.payer,
            self.rent,
            self.system_program,
        ];
        let accounts = [
            AccountMeta::new(self.user.key(), true, false),
            AccountMeta::new(self.user_stats.key(), true, false),
            AccountMeta::new(self.state.key(), true, false),
            AccountMeta::new(self.authority.key(), false, true),
            AccountMeta::new(self.payer.key(), true, true),
            AccountMeta::new(self.rent.key(), false, false),
            AccountMeta::new(self.system_program.key(), false, false),
        ];
        let ix = Instruction {
            program_id: &DRIFT_PROGRAM_ID,
            accounts: &accounts,
            data: &data,
        };
        invoke_signed(&ix, &account_infos, &[signers_seeds])
    }
}
