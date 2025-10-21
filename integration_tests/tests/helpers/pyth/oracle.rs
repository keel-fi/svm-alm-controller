use borsh::{BorshDeserialize, BorshSerialize};
use litesvm::LiteSVM;
use solana_sdk::{account::Account, pubkey::Pubkey};
use svm_alm_controller::constants::anchor_discriminator;

#[derive(BorshSerialize, BorshDeserialize, Copy, Clone, PartialEq, Debug)]
pub enum VerificationLevel {
    Partial { num_signatures: u8 },
    Full,
}

/// Id of a feed producing the message. One feed produces one or more messages.
#[derive(Copy, Clone, Debug, Default, BorshSerialize, BorshDeserialize)]
pub struct FeedId {
    pub id: [u8; 32],
}

#[derive(Copy, Clone, Debug, Default, BorshSerialize, BorshDeserialize)]
pub struct PriceFeedMessage {
    pub feed_id: FeedId,
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    /// The timestamp of this price update in seconds
    pub publish_time: i64,
    /// The timestamp of the previous price update. This field is intended to allow users to
    /// identify the single unique price update for any moment in time:
    /// for any time t, the unique update is the one such that prev_publish_time < t <= publish_time.
    ///
    /// Note that there may not be such an update while we are migrating to the new message-sending logic,
    /// as some price updates on pythnet may not be sent to other chains (because the message-sending
    /// logic may not have triggered). We can solve this problem by making the message-sending mandatory
    /// (which we can do once publishers have migrated over).
    ///
    /// Additionally, this field may be equal to publish_time if the message is sent on a slot where
    /// where the aggregation was unsuccesful. This problem will go away once all publishers have
    /// migrated over to a recent version of pyth-agent.
    pub prev_publish_time: i64,
    pub ema_price: i64,
    pub ema_conf: u64,
}

#[derive(Copy, Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct PriceUpdateV2 {
    pub write_authority: Pubkey,
    pub verification_level: VerificationLevel,
    pub price_message: PriceFeedMessage,
    pub posted_slot: u64,
}

/// Setup mock oracle account for testing
pub fn setup_mock_oracle_account(svm: &mut LiteSVM, oracle_pubkey: &Pubkey, price: i64) {
    // Create a minimal mock oracle account
    let price_update = PriceUpdateV2 {
        write_authority: *oracle_pubkey,
        verification_level: VerificationLevel::Full,
        price_message: PriceFeedMessage {
            feed_id: FeedId { id: [0; 32] },
            price,
            conf: 1,
            exponent: 6,
            publish_time: 1640995200, // Valid timestamp
            prev_publish_time: 1640995200,
            ema_price: 1_000_000,
            ema_conf: 1_000,
        },
        posted_slot: 0,
    };
    let mut oracle_data = Vec::with_capacity(std::mem::size_of::<PriceUpdateV2>() + 8);
    oracle_data.extend_from_slice(&anchor_discriminator("account", "PriceUpdateV2"));
    oracle_data.extend_from_slice(borsh::to_vec(&price_update).unwrap().as_slice());

    svm.set_account(
        *oracle_pubkey,
        Account {
            lamports: u64::MAX,
            rent_epoch: u64::MAX,
            data: oracle_data,
            owner: solana_sdk::pubkey!("FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH"), // pyth program id
            executable: false,
        },
    )
    .unwrap();
}
