use pinocchio::{
    account_info::AccountInfo,
    cpi::invoke_signed,
    instruction::{AccountMeta, Instruction, Signer},
    ProgramResult,
};

use crate::{constants::anchor_discriminator, integrations::drift::constants::DRIFT_PROGRAM_ID};

pub struct InitializeUserStats<'info> {
    pub user_stats: &'info AccountInfo,
    pub state: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub payer: &'info AccountInfo,
    pub rent: &'info AccountInfo,
    pub system_program: &'info AccountInfo,
}
const INIT_USER_STATS_DISC: [u8; 8] = anchor_discriminator("global", "initialize_user_stats");

/// Initialize Drift UserStats account.
/// This only needs to be called ONCE per Controller.
/// NOTE: check for existence before invoking.
pub fn initialize_user_stats(
    accounts: InitializeUserStats,
    signers_seeds: Signer,
) -> ProgramResult {
    let account_infos = [
        accounts.user_stats,
        accounts.state,
        accounts.authority,
        accounts.payer,
        accounts.rent,
        accounts.system_program,
    ];
    let accounts = [
        AccountMeta::new(accounts.user_stats.key(), true, false),
        AccountMeta::new(accounts.state.key(), true, false),
        AccountMeta::new(accounts.authority.key(), false, true),
        AccountMeta::new(accounts.payer.key(), true, true),
        AccountMeta::new(accounts.rent.key(), false, false),
        AccountMeta::new(accounts.system_program.key(), false, false),
    ];
    let ix = Instruction {
        program_id: &DRIFT_PROGRAM_ID,
        accounts: &accounts,
        data: &INIT_USER_STATS_DISC,
    };
    invoke_signed(&ix, &account_infos, &[signers_seeds])
}
