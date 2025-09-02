use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    state::{keel_account::KeelAccount, Oracle},
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
use switchboard_on_demand::{Discriminator, PullFeedAccountData};

define_account_struct! {
    pub struct RefreshOracle<'info> {
        price_feed;
        oracle: mut;
    }
}

pub fn process_refresh_oracle(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!("refresh_oracle");
    let ctx = RefreshOracle::from_accounts(accounts)?;
    let oracle = &mut Oracle::load_and_check(ctx.oracle)?;

    // Read only from first feed in current implementation.
    let feed = &oracle.feeds[0];
    if ctx.price_feed.key().ne(&feed.price_feed) {
        return Err(ProgramError::InvalidAccountData);
    }
    let feed_account = ctx.price_feed.try_borrow_data()?;
    let clock = Clock::get()?;

    match feed.oracle_type {
        0 => {
            if &feed_account[..8] != PullFeedAccountData::DISCRIMINATOR {
                msg!("Invalid PullFeedAccount discriminator");
                return Err(ProgramError::InvalidAccountData);
            }
            let data_source: &PullFeedAccountData = bytemuck::try_from_bytes(&feed_account[8..])
                .map_err(|_| ProgramError::InvalidAccountData)?;
            let price = data_source.result.value;
            let update_slot = data_source.result.slot;

            if update_slot < clock.slot - data_source.max_staleness as u64 {
                log!("update slot {} < current slot {}", update_slot, clock.slot);
                return Err(SvmAlmControllerErrors::StaleOraclePrice.into());
            }

            oracle.value = price;
            oracle.last_update_slot = update_slot;
        }
        _ => {
            return Err(SvmAlmControllerErrors::UnsupportedOracleType.into());
        }
    }

    oracle.save(ctx.oracle)?;

    Ok(())
}
