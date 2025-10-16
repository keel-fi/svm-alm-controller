use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};
use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    pubkey::{self, Pubkey},
};
use svm_alm_controller::constants::anchor_discriminator;
use svm_alm_controller_client::integrations::drift::{
    derive_spot_market_pda, derive_spot_market_vault_pda, DRIFT_PROGRAM_ID,
};

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct HistoricalOracleData {
    /// precision: PRICE_PRECISION
    pub last_oracle_price: i64,
    /// precision: PRICE_PRECISION
    pub last_oracle_conf: u64,
    /// number of slots since last update
    pub last_oracle_delay: i64,
    /// precision: PRICE_PRECISION
    pub last_oracle_price_twap: i64,
    /// precision: PRICE_PRECISION
    pub last_oracle_price_twap_5min: i64,
    /// unix_timestamp of last snapshot
    pub last_oracle_price_twap_ts: i64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct HistoricalIndexData {
    /// precision: PRICE_PRECISION
    pub last_index_bid_price: u64,
    /// precision: PRICE_PRECISION
    pub last_index_ask_price: u64,
    /// precision: PRICE_PRECISION
    pub last_index_price_twap: u64,
    /// precision: PRICE_PRECISION
    pub last_index_price_twap_5min: u64,
    /// unix_timestamp of last snapshot
    pub last_index_price_twap_ts: i64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct PoolBalance {
    /// To get the pool's token amount, you must multiply the scaled balance by the market's cumulative
    /// deposit interest
    /// precision: SPOT_BALANCE_PRECISION
    pub scaled_balance: u128,
    /// The spot market the pool is for
    pub market_index: u16,
    pub padding: [u8; 6],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct InsuranceFund {
    pub vault: Pubkey,
    pub total_shares: u128,
    pub user_shares: u128,
    pub shares_base: u128,     // exponent for lp shares (for rebasing)
    pub unstaking_period: i64, // if_unstaking_period
    pub last_revenue_settle_ts: i64,
    pub revenue_settle_period: i64,
    pub total_factor: u32, // percentage of interest for total insurance
    pub user_factor: u32,  // percentage of interest for user staked insurance
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct SpotMarket {
    /// The address of the spot market. It is a pda of the market index
    pub pubkey: Pubkey,
    /// The oracle used to price the markets deposits/borrows
    pub oracle: Pubkey,
    /// The token mint of the market
    pub mint: Pubkey,
    /// The vault used to store the market's deposits
    /// The amount in the vault should be equal to or greater than deposits - borrows
    pub vault: Pubkey,
    /// The encoded display name for the market e.g. SOL
    pub name: [u8; 32],
    pub historical_oracle_data: HistoricalOracleData,
    pub historical_index_data: HistoricalIndexData,
    /// Revenue the protocol has collected in this markets token
    /// e.g. for SOL-PERP, funds can be settled in usdc and will flow into the USDC revenue pool
    pub revenue_pool: PoolBalance, // in base asset
    /// The fees collected from swaps between this market and the quote market
    /// Is settled to the quote markets revenue pool
    pub spot_fee_pool: PoolBalance,
    /// Details on the insurance fund covering bankruptcies in this markets token
    /// Covers bankruptcies for borrows with this markets token and perps settling in this markets token
    pub insurance_fund: InsuranceFund,
    /// The total spot fees collected for this market
    /// precision: QUOTE_PRECISION
    pub total_spot_fee: u128,
    /// The sum of the scaled balances for deposits across users and pool balances
    /// To convert to the deposit token amount, multiply by the cumulative deposit interest
    /// precision: SPOT_BALANCE_PRECISION
    pub deposit_balance: u128,
    /// The sum of the scaled balances for borrows across users and pool balances
    /// To convert to the borrow token amount, multiply by the cumulative borrow interest
    /// precision: SPOT_BALANCE_PRECISION
    pub borrow_balance: u128,
    /// The cumulative interest earned by depositors
    /// Used to calculate the deposit token amount from the deposit balance
    /// precision: SPOT_CUMULATIVE_INTEREST_PRECISION
    pub cumulative_deposit_interest: u128,
    /// The cumulative interest earned by borrowers
    /// Used to calculate the borrow token amount from the borrow balance
    /// precision: SPOT_CUMULATIVE_INTEREST_PRECISION
    pub cumulative_borrow_interest: u128,
    /// The total socialized loss from borrows, in the mint's token
    /// precision: token mint precision
    pub total_social_loss: u128,
    /// The total socialized loss from borrows, in the quote market's token
    /// preicision: QUOTE_PRECISION
    pub total_quote_social_loss: u128,
    /// no withdraw limits/guards when deposits below this threshold
    /// precision: token mint precision
    pub withdraw_guard_threshold: u64,
    /// The max amount of token deposits in this market
    /// 0 if there is no limit
    /// precision: token mint precision
    pub max_token_deposits: u64,
    /// 24hr average of deposit token amount
    /// precision: token mint precision
    pub deposit_token_twap: u64,
    /// 24hr average of borrow token amount
    /// precision: token mint precision
    pub borrow_token_twap: u64,
    /// 24hr average of utilization
    /// which is borrow amount over token amount
    /// precision: SPOT_UTILIZATION_PRECISION
    pub utilization_twap: u64,
    /// Last time the cumulative deposit and borrow interest was updated
    pub last_interest_ts: u64,
    /// Last time the deposit/borrow/utilization averages were updated
    pub last_twap_ts: u64,
    /// The time the market is set to expire. Only set if market is in reduce only mode
    pub expiry_ts: i64,
    /// Spot orders must be a multiple of the step size
    /// precision: token mint precision
    pub order_step_size: u64,
    /// Spot orders must be a multiple of the tick size
    /// precision: PRICE_PRECISION
    pub order_tick_size: u64,
    /// The minimum order size
    /// precision: token mint precision
    pub min_order_size: u64,
    /// The maximum spot position size
    /// if the limit is 0, there is no limit
    /// precision: token mint precision
    pub max_position_size: u64,
    /// Every spot trade has a fill record id. This is the next id to use
    pub next_fill_record_id: u64,
    /// Every deposit has a deposit record id. This is the next id to use
    pub next_deposit_record_id: u64,
    /// The initial asset weight used to calculate a deposits contribution to a users initial total collateral
    /// e.g. if the asset weight is .8, $100 of deposits contributes $80 to the users initial total collateral
    /// precision: SPOT_WEIGHT_PRECISION
    pub initial_asset_weight: u32,
    /// The maintenance asset weight used to calculate a deposits contribution to a users maintenance total collateral
    /// e.g. if the asset weight is .9, $100 of deposits contributes $90 to the users maintenance total collateral
    /// precision: SPOT_WEIGHT_PRECISION
    pub maintenance_asset_weight: u32,
    /// The initial liability weight used to calculate a borrows contribution to a users initial margin requirement
    /// e.g. if the liability weight is .9, $100 of borrows contributes $90 to the users initial margin requirement
    /// precision: SPOT_WEIGHT_PRECISION
    pub initial_liability_weight: u32,
    /// The maintenance liability weight used to calculate a borrows contribution to a users maintenance margin requirement
    /// e.g. if the liability weight is .8, $100 of borrows contributes $80 to the users maintenance margin requirement
    /// precision: SPOT_WEIGHT_PRECISION
    pub maintenance_liability_weight: u32,
    /// The initial margin fraction factor. Used to increase liability weight/decrease asset weight for large positions
    /// precision: MARGIN_PRECISION
    pub imf_factor: u32,
    /// The fee the liquidator is paid for taking over borrow/deposit
    /// precision: LIQUIDATOR_FEE_PRECISION
    pub liquidator_fee: u32,
    /// The fee the insurance fund receives from liquidation
    /// precision: LIQUIDATOR_FEE_PRECISION
    pub if_liquidation_fee: u32,
    /// The optimal utilization rate for this market.
    /// Used to determine the markets borrow rate
    /// precision: SPOT_UTILIZATION_PRECISION
    pub optimal_utilization: u32,
    /// The borrow rate for this market when the market has optimal utilization
    /// precision: SPOT_RATE_PRECISION
    pub optimal_borrow_rate: u32,
    /// The borrow rate for this market when the market has 1000 utilization
    /// precision: SPOT_RATE_PRECISION
    pub max_borrow_rate: u32,
    /// The market's token mint's decimals. To from decimals to a precision, 10^decimals
    pub decimals: u32,
    pub market_index: u16,
    /// Whether or not spot trading is enabled
    pub orders_enabled: u8,
    pub oracle_source: u8,
    pub status: u8,
    /// The asset tier affects how a deposit can be used as collateral and the priority for a borrow being liquidated
    pub asset_tier: u8,
    pub paused_operations: u8,
    pub if_paused_operations: u8,
    pub fee_adjustment: i16,
    /// What fraction of max_token_deposits
    /// disabled when 0, 1 => 1/10000 => .01% of max_token_deposits
    /// precision: X/10000
    pub max_token_borrows_fraction: u16,
    /// For swaps, the amount of token loaned out in the begin_swap ix
    /// precision: token mint precision
    pub flash_loan_amount: u64,
    /// For swaps, the amount in the users token account in the begin_swap ix
    /// Used to calculate how much of the token left the system in end_swap ix
    /// precision: token mint precision
    pub flash_loan_initial_token_amount: u64,
    /// The total fees received from swaps
    /// precision: token mint precision
    pub total_swap_fee: u64,
    /// When to begin scaling down the initial asset weight
    /// disabled when 0
    /// precision: QUOTE_PRECISION
    pub scale_initial_asset_weight_start: u64,
    /// The min borrow rate for this market when the market regardless of utilization
    /// 1 => 1/200 => .5%
    /// precision: X/200
    pub min_borrow_rate: u8,
    /// fuel multiplier for spot deposits
    /// precision: 10
    pub fuel_boost_deposits: u8,
    /// fuel multiplier for spot borrows
    /// precision: 10
    pub fuel_boost_borrows: u8,
    /// fuel multiplier for spot taker
    /// precision: 10
    pub fuel_boost_taker: u8,
    /// fuel multiplier for spot maker
    /// precision: 10
    pub fuel_boost_maker: u8,
    /// fuel multiplier for spot insurance stake
    /// precision: 10
    pub fuel_boost_insurance: u8,
    pub token_program_flag: u8,
    pub pool_id: u8,
    // padding expanded into 5 chunks to be Pod
    pub padding_1: [u8; 8],
    pub padding_2: [u8; 8],
    pub padding_3: [u8; 8],
    pub padding_4: [u8; 8],
    pub padding_5: [u8; 8],
}
impl SpotMarket {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "SpotMarket");
}

/// Setup Drift SpotMarket state in LiteSvm giving full control over state.
///
/// If anything is not set correctly for a subsequent test, either:
/// - IF applicable to all tests, mutate state with a set value here
/// - ELSE IF requires variable values for testing, add a argument
///     and mutate state set from the arg.
pub fn set_drift_spot_market(svm: &mut LiteSVM, market_index: u16, mint: Option<Pubkey>) -> Pubkey {
    let pubkey = derive_spot_market_pda(market_index);

    let mut spot_market = SpotMarket::default();
    // -- Update state variables
    spot_market.pubkey = pubkey; // Set the pubkey field to the actual PDA
    spot_market.market_index = market_index;
    if let Some(mint) = mint {
        spot_market.mint = mint;
    }

    // Set up vault account (the spot market vault PDA)
    let vault_pubkey =
        svm_alm_controller_client::integrations::drift::derive_spot_market_vault_pda(market_index);
    spot_market.vault = vault_pubkey;

    // Set up oracle account (mock oracle for testing)
    let oracle_pubkey = Pubkey::new_unique();
    spot_market.oracle = oracle_pubkey;

    // Set up insurance fund vault (mock insurance fund for testing)
    let insurance_fund_vault = Pubkey::new_unique();
    spot_market.insurance_fund.vault = insurance_fund_vault;

    // Set important fields for the Drift program to recognize this as a valid spot market
    spot_market.status = 1; // Active status
    spot_market.orders_enabled = 1; // Enable orders
    spot_market.asset_tier = 1; // Set asset tier
    spot_market.decimals = 6; // Set decimals (matching our token mint)
    spot_market.oracle_source = 7; // Set oracle source to pyth pull
    spot_market.cumulative_deposit_interest = 1000000000000000000; // Set cumulative deposit interest to 1
    spot_market.cumulative_borrow_interest = 1000000000000000000; // Set cumulative borrow interest to 1

    // Set market_index in PoolBalance structs - this is critical for Drift validation
    spot_market.revenue_pool.market_index = market_index;
    spot_market.spot_fee_pool.market_index = market_index;

    let mut state_data = Vec::with_capacity(std::mem::size_of::<SpotMarket>() + 8);
    state_data.extend_from_slice(&SpotMarket::DISCRIMINATOR);
    state_data.extend_from_slice(&bytemuck::bytes_of(&spot_market));

    svm.set_account(
        pubkey,
        Account {
            lamports: u64::MAX,
            rent_epoch: u64::MAX,
            data: state_data,
            owner: DRIFT_PROGRAM_ID,
            executable: false,
        },
    )
    .unwrap();

    pubkey
}

/// Setup Drift SpotMarket Vault token account in LiteSvm.
/// This creates the actual token account that holds the market's deposits.
pub fn setup_drift_spot_market_vault(
    svm: &mut LiteSVM,
    market_index: u16,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Pubkey {
    let vault_pubkey =
        svm_alm_controller_client::integrations::drift::derive_spot_market_vault_pda(market_index);

    // Import the setup_token_account function
    use crate::helpers::spl::setup_token_account;

    setup_token_account(
        svm,
        &vault_pubkey,
        mint,
        &vault_pubkey, // The vault PDA is the owner of its own token account
        0,             // Start with 0 tokens
        token_program,
        None,
    );

    vault_pubkey
}

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
pub fn setup_mock_oracle_account(svm: &mut LiteSVM, oracle_pubkey: &Pubkey) {
    use solana_sdk::account::Account;

    // Create a minimal mock oracle account
    let price_update = PriceUpdateV2 {
        write_authority: *oracle_pubkey,
        verification_level: VerificationLevel::Full,
        price_message: PriceFeedMessage {
            feed_id: FeedId { id: [0; 32] },
            price: 0,
            conf: 0,
            exponent: 6,
            publish_time: 0,
            prev_publish_time: 0,
            ema_price: 0,
            ema_conf: 0,
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

/// Setup mock insurance fund account for testing
pub fn setup_mock_insurance_fund_account(svm: &mut LiteSVM, insurance_fund_pubkey: &Pubkey) {
    use solana_sdk::account::Account;

    // Create a minimal mock insurance fund account
    let insurance_fund_data = vec![0u8; 64]; // Minimal insurance fund data

    svm.set_account(
        *insurance_fund_pubkey,
        Account {
            lamports: u64::MAX,
            rent_epoch: u64::MAX,
            data: insurance_fund_data,
            owner: DRIFT_PROGRAM_ID, // Owned by Drift program
            executable: false,
        },
    )
    .unwrap();
}
