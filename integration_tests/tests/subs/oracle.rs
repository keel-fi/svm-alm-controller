use std::error::Error;

use borsh::BorshSerialize;
use bytemuck::Zeroable;
use litesvm::LiteSVM;
use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};

use svm_alm_controller::state::AccountDiscriminators;
use svm_alm_controller_client::{generated::accounts::Oracle, SVM_ALM_CONTROLLER_ID};
use switchboard_on_demand::{Discriminator, OracleSubmission, PullFeedAccountData};

pub fn set_oracle_price(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    price: i128,
) -> Result<(), Box<dyn Error>> {
    let clock: Clock = svm.get_sysvar();
    let slot = clock.slot;

    let mut feed_data = PullFeedAccountData::zeroed();
    feed_data.authority = Pubkey::new_unique();
    feed_data.queue = Pubkey::new_unique();
    feed_data.min_responses = 1;
    feed_data.min_sample_size = 1;
    feed_data.max_staleness = 150u32;
    feed_data.result.debug_only_force_override(price, slot);
    feed_data.result.submission_idx = 0;
    feed_data.submissions[0] = OracleSubmission {
        oracle: Pubkey::new_unique(),
        slot,
        landed_at: slot,
        value: price,
    };
    feed_data.submission_timestamps[0] = clock.unix_timestamp;

    let mut serialized = Vec::with_capacity(8 + std::mem::size_of::<PullFeedAccountData>());
    serialized.extend_from_slice(&PullFeedAccountData::DISCRIMINATOR);
    serialized.extend_from_slice(bytemuck::bytes_of(&feed_data));

    svm.set_account(
        *pubkey,
        Account {
            lamports: 1_000_000_000,
            data: serialized,
            owner: SVM_ALM_CONTROLLER_ID,
            executable: false,
            rent_epoch: u64::MAX,
        },
    )?;

    Ok(())
}
