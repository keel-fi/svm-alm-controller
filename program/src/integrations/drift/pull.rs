use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::IntegrationConfig,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    instructions::PushArgs,
    integrations::drift::{constants::DRIFT_PROGRAM_ID, cpi::Deposit},
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

define_account_struct! {
    pub struct PullDriftAccounts<'info> {
        state: @owner(DRIFT_PROGRAM_ID);
        user: mut @owner(DRIFT_PROGRAM_ID);
        user_stats: mut @owner(DRIFT_PROGRAM_ID);
        spot_market_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        // TODO unsure if this is an empty account
        drift_signer;
        user_token_account: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_vault: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
        @remaining_accounts as remaining_accounts;
    }
}

impl<'info> PullDriftAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        accounts_infos: &'info [AccountInfo],
        spot_market_index: u16,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;
        let config = match config {
            IntegrationConfig::Drift(config) => config,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        if spot_market_index != config.spot_market_index {
            msg!("spot_market_index: does not match config");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(ctx)
    }
}
pub fn process_pull_drift(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> ProgramResult {
    msg!("process_pull_drift");

    Ok(())
}
