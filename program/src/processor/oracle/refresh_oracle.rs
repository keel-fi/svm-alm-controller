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

    // Read only from first feed in current implementation.
    let feed = &oracle.feeds[0];
    if ctx.price_feed.key().ne(&feed.price_feed) {
        return Err(ProgramError::InvalidAccountData);
    }
    let feed_account = ctx.price_feed.try_borrow_data()?;
    let clock = Clock::get()?;

    match feed.oracle_type {
        0 => {
            let data_source: &PullFeedAccountData = bytemuck::from_bytes(&feed_account[8..]);
            let price = data_source.result.value;
            let update_slot = data_source.result.slot;

            if update_slot < clock.slot - data_source.max_staleness as u64 {
                log!("update slot {} < current slot {}", update_slot, clock.slot);
                return Err(SvmAlmControllerErrors::StaleOraclePrice.into());
            }

            // Let P = precision of price and X = Price in decimals
            // Price is stored in data feed as X * (10^P).
            // By inverting, we want to get 1/X * (10^P)
            // = 10^P / X = 10^(2*P) / (X * 10^P)
            if feed.invert_price {
                oracle.value = 10_i128
                    .checked_pow(PRECISION * 2)
                    .unwrap()
                    .checked_div(price)
                    .unwrap();
            } else {
                oracle.value = price;
            }
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
