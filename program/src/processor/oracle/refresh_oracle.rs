use crate::{
    error::SvmAlmControllerErrors,
    state::{nova_account::NovaAccount, Oracle},
};
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_log::log;
use switchboard_on_demand::{PullFeedAccountData, PRECISION};

pub struct RefreshOracle<'info> {
    pub price_feed: &'info AccountInfo,
    pub oracle: &'info AccountInfo,
}

impl<'info> RefreshOracle<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            price_feed: &accounts[0],
            oracle: &accounts[1],
        };
        if !ctx.oracle.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(ctx)
    }
}

pub fn process_refresh_oracle(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!("refresh_oracle");
    let ctx = RefreshOracle::from_accounts(accounts)?;
    let oracle = &mut Oracle::load_and_check_mut(ctx.oracle)?;

    let feed_account = ctx.price_feed.try_borrow_data()?;
    let clock = Clock::get()?;

    match oracle.oracle_type {
        0 => {
            let feed: &PullFeedAccountData = bytemuck::from_bytes(&feed_account[8..]);
            let price = feed.result.value;
            let update_slot = feed.result.slot;

            if update_slot < clock.slot - feed.max_staleness as u64 {
                log!("update slot {} < current slot {}", update_slot, clock.slot);
                return Err(SvmAlmControllerErrors::StaleOraclePrice.into());
            }

            oracle.value = price;
            oracle.precision = PRECISION;
            oracle.last_update_slot = update_slot;
        }
        _ => {
            return Err(SvmAlmControllerErrors::UnsupportedOracleType.into());
        }
    }

    oracle.save(ctx.oracle)?;

    Ok(())
}
