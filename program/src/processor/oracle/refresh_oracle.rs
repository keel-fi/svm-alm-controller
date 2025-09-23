use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    state::{keel_account::KeelAccount, Oracle},
};
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use switchboard_on_demand::{Discriminator, PullFeedAccountData};

define_account_struct! {
    pub struct RefreshOracle<'info> {
        price_feed;
        oracle: mut, @owner(crate::ID);
    }
}

pub fn process_refresh_oracle(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!("refresh_oracle");
    let ctx = RefreshOracle::from_accounts(accounts)?;

    // We do not require Controller checks and deem it ok to refresh
    // the Oracle state even when the associated Controller is in a Frozen
    // state. These updates are purely informational and have no impact
    // on Controller related assets.

    // Load and check Oracle state
    let mut oracle = Oracle::load_and_check(ctx.oracle, None, None)?;

    // Read only from first feed in current implementation.
    let feed = &oracle.feeds[0];
    if ctx.price_feed.key().ne(&feed.price_feed) {
        return Err(ProgramError::InvalidAccountData);
    }
    let feed_account = ctx.price_feed.try_borrow_data()?;

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

            oracle.value = price;
            oracle.last_update_slot = update_slot;
        }
        _ => {
            return Err(SvmAlmControllerErrors::UnsupportedOracleType.into());
        }
    }

    // NOTE: we pureposefully do NOT emit an event here. It has been deemed
    // excessive to emit an event for every price change the Oracle has. Offchain
    // services may simply listen to the account state changes directly.

    oracle.save(ctx.oracle)?;

    Ok(())
}
