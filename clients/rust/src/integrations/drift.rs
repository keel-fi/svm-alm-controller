use bytemuck::{Pod, Zeroable};
use solana_pubkey::{pubkey, Pubkey};

use svm_alm_controller::constants::anchor_discriminator;

pub const DRIFT_PROGRAM_ID: Pubkey = pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

// Import the SpotMarket struct from the integration tests helpers
// This is a temporary solution - ideally we'd have this in a shared location
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
struct HistoricalOracleData {
    pub last_oracle_price: i64,
    pub last_oracle_conf: u64,
    pub last_oracle_delay: i64,
    pub last_oracle_price_twap: i64,
    pub last_oracle_price_twap_5min: i64,
    pub last_oracle_price_twap_ts: i64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
struct HistoricalIndexData {
    pub last_index_bid_price: u64,
    pub last_index_ask_price: u64,
    pub last_index_price_twap: u64,
    pub last_index_price_twap_5min: u64,
    pub last_index_price_twap_ts: i64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct PoolBalance {
    pub scaled_balance: u128,
    pub market_index: u16,
    pub padding: [u8; 6],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct InsuranceFund {
    pub vault: Pubkey,
    pub total_shares: u128,
    pub user_shares: u128,
    pub shares_base: u128,
    pub unstaking_period: i64,
    pub last_revenue_settle_ts: i64,
    pub revenue_settle_period: i64,
    pub total_factor: u32,
    pub user_factor: u32,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct SpotMarket {
    pub pubkey: Pubkey,
    pub oracle: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub name: [u8; 32],
    pub historical_oracle_data: HistoricalOracleData,
    pub historical_index_data: HistoricalIndexData,
    pub revenue_pool: PoolBalance,
    pub spot_fee_pool: PoolBalance,
    pub insurance_fund: InsuranceFund,
    pub total_spot_fee: u128,
    pub deposit_balance: u128,
    pub borrow_balance: u128,
    pub cumulative_deposit_interest: u128,
    pub cumulative_borrow_interest: u128,
    pub total_social_loss: u128,
    pub total_quote_social_loss: u128,
    pub withdraw_guard_threshold: u64,
    pub max_token_deposits: u64,
    pub deposit_token_twap: u64,
    pub borrow_token_twap: u64,
    pub utilization_twap: u64,
    pub last_interest_ts: u64,
    pub last_twap_ts: u64,
    pub expiry_ts: i64,
    pub order_step_size: u64,
    pub order_tick_size: u64,
    pub min_order_size: u64,
    pub max_position_size: u64,
    pub next_fill_record_id: u64,
    pub next_deposit_record_id: u64,
    pub initial_asset_weight: u32,
    pub maintenance_asset_weight: u32,
    pub initial_liability_weight: u32,
    pub maintenance_liability_weight: u32,
    pub imf_factor: u32,
    pub liquidator_fee: u32,
    pub if_liquidation_fee: u32,
    pub optimal_utilization: u32,
    pub optimal_borrow_rate: u32,
    pub max_borrow_rate: u32,
    pub decimals: u32,
    pub market_index: u16,
    pub orders_enabled: u8,
    pub oracle_source: u8,
    pub status: u8,
    pub asset_tier: u8,
    pub paused_operations: u8,
    pub if_paused_operations: u8,
    pub fee_adjustment: i16,
    pub max_token_borrows_fraction: u16,
    pub flash_loan_amount: u64,
    pub flash_loan_initial_token_amount: u64,
    pub total_swap_fee: u64,
    pub scale_initial_asset_weight_start: u64,
    pub min_borrow_rate: u8,
    pub fuel_boost_deposits: u8,
    pub fuel_boost_borrows: u8,
    pub fuel_boost_taker: u8,
    pub fuel_boost_maker: u8,
    pub fuel_boost_insurance: u8,
    pub token_program_flag: u8,
    pub pool_id: u8,
    pub padding_1: [u8; 8],
    pub padding_2: [u8; 8],
    pub padding_3: [u8; 8],
    pub padding_4: [u8; 8],
    pub padding_5: [u8; 8],
}

impl SpotMarket {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "SpotMarket");
}

/// Extract oracle and insurance fund addresses from spot market account data
pub fn extract_spot_market_data(
    spot_market_account_data: &[u8],
) -> Result<SpotMarket, Box<dyn std::error::Error>> {
    // Skip the 8-byte discriminator
    if spot_market_account_data.len() < 8 {
        return Err("Account data too short".into());
    }

    let market_data = &spot_market_account_data[8..];

    // Parse the SpotMarket struct using bytemuck
    let spot_market = bytemuck::try_from_bytes::<SpotMarket>(market_data)
        .map_err(|e| format!("Failed to parse spot market data: {}", e))?;

    Ok(*spot_market)
}

/// Derives State PDA
pub fn derive_state_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"drift_state"], &DRIFT_PROGRAM_ID).0
}

/// Derives UserStats PDA
pub fn derive_user_stats_pda(authority: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user_stats", authority.as_ref()], &DRIFT_PROGRAM_ID).0
}

/// Derives User subaccount PDA
pub fn derive_user_pda(authority: &Pubkey, sub_account_id: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"user",
            authority.as_ref(),
            sub_account_id.to_le_bytes().as_ref(),
        ],
        &DRIFT_PROGRAM_ID,
    )
    .0
}

/// Derives SpotMarket PDA
pub fn derive_spot_market_pda(market_index: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[b"spot_market", market_index.to_le_bytes().as_ref()],
        &DRIFT_PROGRAM_ID,
    )
    .0
}

/// Derives SpotMarket Vault PDA
pub fn derive_spot_market_vault_pda(market_index: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[b"spot_market_vault", market_index.to_le_bytes().as_ref()],
        &DRIFT_PROGRAM_ID,
    )
    .0
}
