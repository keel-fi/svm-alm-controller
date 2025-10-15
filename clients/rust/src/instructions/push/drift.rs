use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use bytemuck::{Pod, Zeroable};

use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::PushBuilder, types::PushArgs},
    integrations::drift::{
        derive_spot_market_pda, derive_spot_market_vault_pda, derive_state_pda, derive_user_pda, derive_user_stats_pda,
    },
};

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
struct PoolBalance {
    pub scaled_balance: u128,
    pub market_index: u16,
    pub padding: [u8; 6],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
struct InsuranceFund {
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
struct SpotMarket {
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

/// Extract oracle and insurance fund addresses from spot market account data
fn extract_spot_market_data(spot_market_account_data: &[u8]) -> Result<SpotMarket, Box<dyn std::error::Error>> {
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

/// Instruction generation for Drift "Push".
pub fn create_drift_push_instruction(
    controller: &Pubkey,
    super_authority: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
    reserve_vault: &Pubkey,
    user_token_account: &Pubkey,
    token_program: &Pubkey,
    spot_market_index: u16,
    sub_account_id: u16,
    amount: u64,
    reduce_only: bool,
    spot_market_account_data: &[u8],
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let controller_authority = derive_controller_authority_pda(controller);
    let permission_pda = derive_permission_pda(controller, super_authority);
    let drift_state_pda = derive_state_pda();
    let drift_user_stats_pda = derive_user_stats_pda(&controller_authority);
    let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
    let drift_spot_market_vault_pda = derive_spot_market_vault_pda(spot_market_index);

    // Extract oracle and insurance fund addresses from spot market data
    let spot_market = extract_spot_market_data(spot_market_account_data)?;

    let oracle_pubkey = spot_market.oracle;

    let remaining_accounts = [
        AccountMeta {
            pubkey: drift_state_pda,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: drift_user_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: drift_user_stats_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: drift_spot_market_vault_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *user_token_account, // user_token_account - controller authority's ATA
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *token_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *reserve_vault, // reserve vault for balance sync
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: oracle_pubkey,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: spot_market.pubkey,
            is_signer: false,
            is_writable: true,
        },
    ];

    let instruction = PushBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*super_authority)
        .permission(permission_pda)
        .integration(*integration)
        .reserve_a(*reserve)
        .reserve_b(*reserve)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .push_args(PushArgs::Drift {
            market_index: spot_market_index,
            amount,
            reduce_only,
        })
        .add_remaining_accounts(&remaining_accounts)
        .instruction();
        
    Ok(instruction)
}
