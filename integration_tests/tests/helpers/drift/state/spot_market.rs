use crate::{
    helpers::spl::setup_token_account,
    subs::{edit_token_amount, get_mint},
};
use litesvm::LiteSVM;
use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};
use svm_alm_controller_client::integrations::drift::{
    derive_drift_signer, derive_spot_market_pda, SpotMarket, DRIFT_PROGRAM_ID,
};

/// Setup Drift SpotMarket state in LiteSvm giving full control over state.
///
/// If anything is not set correctly for a subsequent test, either:
/// - IF applicable to all tests, mutate state with a set value here
/// - ELSE IF requires variable values for testing, add a argument
///     and mutate state set from the arg.
pub fn set_drift_spot_market(
    svm: &mut LiteSVM,
    market_index: u16,
    mint: &Pubkey,
    oracle_price: i64,
    pool_id: u8,
) -> SpotMarket {
    let spot_market_pubkey = derive_spot_market_pda(market_index);

    let mut spot_market = SpotMarket::default();
    spot_market.pool_id = pool_id;
    // -- Update state variables
    spot_market.pubkey = spot_market_pubkey; // Set the pubkey field to the actual PDA
    spot_market.market_index = market_index;
    // Set TWAP oracle price to the provided oracle_price
    spot_market.historical_oracle_data.last_oracle_price_twap = oracle_price;
    spot_market
        .historical_oracle_data
        .last_oracle_price_twap_5min = oracle_price;
    spot_market.mint = *mint;
    let mint_account = get_mint(svm, &spot_market.mint);
    spot_market.decimals = mint_account.decimals as u32;

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
    spot_market.oracle_source = 7; // Set oracle source to pyth pull
    spot_market.cumulative_deposit_interest = 10_000_000_000; // Set cumulative deposit interest to 1
    spot_market.cumulative_borrow_interest = 10_000_000_000; // Set cumulative borrow interest to 1

    // Set market_index in PoolBalance structs - this is critical for Drift validation
    spot_market.revenue_pool.market_index = market_index;
    spot_market.spot_fee_pool.market_index = market_index;

    let mut state_data = Vec::with_capacity(std::mem::size_of::<SpotMarket>() + 8);
    state_data.extend_from_slice(&SpotMarket::DISCRIMINATOR);
    state_data.extend_from_slice(&bytemuck::bytes_of(&spot_market));

    svm.set_account(
        spot_market_pubkey,
        Account {
            lamports: u64::MAX,
            rent_epoch: u64::MAX,
            data: state_data,
            owner: DRIFT_PROGRAM_ID,
            executable: false,
        },
    )
    .unwrap();

    spot_market
}

pub fn set_drift_spot_market_pool_id(svm: &mut LiteSVM, spot_market_pk: &Pubkey, new_pool_id: u8) {
    let mut spot_market_account = svm.get_account(spot_market_pk).unwrap();
    let spot_market_data = &mut spot_market_account.data[8..]; // Skip discriminator
    let spot_market = bytemuck::try_from_bytes_mut::<SpotMarket>(spot_market_data).unwrap();
    spot_market.pool_id = new_pool_id;
    // Allow any amount of withdraws
    spot_market.withdraw_guard_threshold = u64::MAX;

    svm.set_account(
        *spot_market_pk,
        Account {
            lamports: u64::MAX,
            rent_epoch: u64::MAX,
            data: vec![
                SpotMarket::DISCRIMINATOR.to_vec(),
                bytemuck::bytes_of(spot_market).to_vec(),
            ]
            .concat(),
            owner: DRIFT_PROGRAM_ID,
            executable: false,
        },
    )
    .unwrap();
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
    let owner = derive_drift_signer();

    setup_token_account(
        svm,
        &vault_pubkey,
        mint,
        &owner, // owner must be "drift_signer"
        0,      // Start with 0 tokens
        token_program,
        None,
    );

    vault_pubkey
}

/// Advances the SVM Clock by 1 year, which allows the exact amount of interest
/// in `spot_market_accrue_cumulative_interest` to accrue.
/// NOTE: this must be called ONLY ONCE before the next TX
pub fn advance_clock_1_drift_year_to_accumulate_interest(svm: &mut LiteSVM) {
    let drift_one_year = 31536000;

    // set clock 1 year in the future to accrue interest.
    // This makes calculatons easier and shouldn't have impact
    // on tests.
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += drift_one_year;
    svm.set_sysvar(&clock);
}

/// Increments the SpotMarket's cumulative_deposit_interest by the given basis points.
pub fn spot_market_accrue_cumulative_interest(
    svm: &mut LiteSVM,
    market_index: u16,
    deposit_interest_bps: u16,
) {
    let spot_market_pubkey = derive_spot_market_pda(market_index);
    let mut spot_market_account = svm.get_account(&spot_market_pubkey).unwrap();
    let spot_market_data = &mut spot_market_account.data[8..]; // Skip discriminator
    let spot_market = bytemuck::try_from_bytes_mut::<SpotMarket>(spot_market_data).unwrap();

    // Allow max withdraw at any time
    spot_market.withdraw_guard_threshold = u64::MAX;
    // known as `SPOT_UTILIZATION_PRECISION` & `SPOT_RATE_PRECISION`
    let drift_scale_factor: u128 = 1_000_000;

    // set utilization and rate parameters
    let scale_u32: u32 = drift_scale_factor.try_into().unwrap();
    // rate when the market is at optimal utilization; 25%
    spot_market.optimal_utilization = 25 * scale_u32;
    // Scaled by 400 to adjust for 25% utilization
    spot_market.optimal_borrow_rate =
        (400 * u64::from(deposit_interest_bps) * u64::from(scale_u32) / 10_000)
            .try_into()
            .unwrap();

    // scale up deposit such that we can withdraw without problems
    spot_market.deposit_balance = spot_market.deposit_balance * 2;
    // Set borrow rate to quarter of deposit to match optimal rate.
    // This makes calculations easier
    spot_market.borrow_balance = spot_market.deposit_balance / 4;

    // Add more tokens to the SpotMarket vault
    edit_token_amount(
        svm,
        &spot_market.vault,
        spot_market.deposit_balance.try_into().unwrap(),
    )
    .unwrap();

    svm.set_account(spot_market_pubkey, spot_market_account)
        .unwrap();
}

/// Setup mock insurance fund account for testing
pub fn setup_mock_insurance_fund_account(svm: &mut LiteSVM, insurance_fund_pubkey: &Pubkey) {
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
