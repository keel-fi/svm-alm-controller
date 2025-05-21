use std::error::Error;

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::Zeroable;
use litesvm::LiteSVM;
use solana_sdk::{
    account::Account, clock::Clock, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_program, transaction::Transaction,
};

use svm_alm_controller::state::AccountDiscriminators;
use svm_alm_controller_client::{
    generated::{
        accounts::Oracle,
        instructions::{InitializeOracleBuilder, RefreshOracleBuilder},
    },
    SVM_ALM_CONTROLLER_ID,
};
use switchboard_on_demand::{Discriminator, OracleSubmission, PullFeedAccountData};

pub fn derive_oracle_pda(feed: &Pubkey) -> Pubkey {
    let (controller_pda, _controller_bump) = Pubkey::find_program_address(
        &[b"oracle", &feed.to_bytes()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    controller_pda
}

pub fn fetch_oracle_account(
    svm: &LiteSVM,
    oracle_pda: &Pubkey,
) -> Result<Option<Oracle>, Box<dyn Error>> {
    let oracle_info = svm.get_account(oracle_pda);
    match oracle_info {
        Some(info) => {
            if info.data.is_empty() {
                Ok(None)
            } else {
                Oracle::try_from_slice(&info.data[1..])
                    .map(Some)
                    .map_err(Into::into)
            }
        }
        None => Ok(None),
    }
}

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

pub fn initalize_oracle(
    svm: &mut LiteSVM,
    payer: &Keypair,
    price_feed: &Pubkey,
    oracle_type: u8,
) -> Result<(), Box<dyn Error>> {
    let oracle_pda = derive_oracle_pda(&price_feed);

    // Initialize Oracle account
    let ixn = InitializeOracleBuilder::new()
        .oracle(oracle_pda)
        .price_feed(*price_feed)
        .system_program(system_program::ID)
        .payer(payer.pubkey())
        .oracle_type(oracle_type)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );
    let tx_result = svm.send_transaction(txn);
    assert!(tx_result.is_ok(), "Transaction failed to execute");
    Ok(())
}

pub fn refresh_oracle(
    svm: &mut LiteSVM,
    payer: &Keypair,
    price_feed: &Pubkey,
) -> Result<(), Box<dyn Error>> {
    let oracle_pda = derive_oracle_pda(&price_feed);

    let ixn = RefreshOracleBuilder::new()
        .oracle(oracle_pda)
        .price_feed(*price_feed)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );
    let tx_result = svm.send_transaction(txn);
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    Ok(())
}
