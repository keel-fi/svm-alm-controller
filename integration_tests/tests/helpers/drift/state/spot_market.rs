use crate::helpers::spl::setup_token_account;
use litesvm::LiteSVM;
use solana_sdk::{account::Account, program_pack::Pack, pubkey::Pubkey};
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
    mint: Option<Pubkey>,
    oracle_price: i64,
) -> Pubkey {
    let pubkey = derive_spot_market_pda(market_index);

    let mut spot_market = SpotMarket::default();
    // -- Update state variables
    spot_market.pubkey = pubkey; // Set the pubkey field to the actual PDA
    spot_market.market_index = market_index;
    // Set TWAP oracle price to the provided oracle_price
    spot_market.historical_oracle_data.last_oracle_price_twap = oracle_price;
    spot_market
        .historical_oracle_data
        .last_oracle_price_twap_5min = oracle_price;
    if let Some(mint) = mint {
        spot_market.mint = mint;
        let mint_account = svm.get_account(&spot_market.mint).unwrap();
        let mint_account = spl_token::state::Mint::unpack(&mint_account.data).unwrap();
        spot_market.decimals = mint_account.decimals as u32;
    } else {
        spot_market.decimals = 6;
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
