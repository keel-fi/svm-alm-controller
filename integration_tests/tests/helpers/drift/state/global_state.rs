use std::u64;

use bytemuck::{Pod, Zeroable};
use litesvm::LiteSVM;
use solana_program::example_mocks::{solana_keypair::Keypair, solana_signer::Signer};
use solana_sdk::{account::Account, pubkey::Pubkey};
use svm_alm_controller::constants::anchor_discriminator;
use svm_alm_controller_client::integrations::drift::{derive_state_pda, DRIFT_PROGRAM_ID};

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct FeeTier {
    pub fee_numerator: u32,
    pub fee_denominator: u32,
    pub maker_rebate_numerator: u32,
    pub maker_rebate_denominator: u32,
    pub referrer_reward_numerator: u32,
    pub referrer_reward_denominator: u32,
    pub referee_fee_numerator: u32,
    pub referee_fee_denominator: u32,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct OrderFillerRewardStructure {
    pub reward_numerator: u32,
    pub reward_denominator: u32,
    pub time_based_reward_lower_bound: u128, // minimum filler reward for time-based reward
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct FeeStructure {
    pub fee_tiers: [FeeTier; 10],
    pub filler_reward_structure: OrderFillerRewardStructure,
    pub referrer_reward_epoch_upper_bound: u64,
    pub flat_filler_fee: u64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct PriceDivergenceGuardRails {
    pub mark_oracle_percent_divergence: u64,
    pub oracle_twap_5min_percent_divergence: u64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct ValidityGuardRails {
    pub slots_before_stale_for_amm: i64,
    pub slots_before_stale_for_margin: i64,
    pub confidence_interval_max_size: u64,
    pub too_volatile_ratio: i64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct OracleGuardRails {
    pub price_divergence: PriceDivergenceGuardRails,
    pub validity: ValidityGuardRails,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct State {
    pub admin: Pubkey,
    pub whitelist_mint: Pubkey,
    pub discount_mint: Pubkey,
    pub signer: Pubkey,
    pub srm_vault: Pubkey,
    pub perp_fee_structure: FeeStructure,
    pub spot_fee_structure: FeeStructure,
    pub oracle_guard_rails: OracleGuardRails,
    pub number_of_authorities: u64,
    pub number_of_sub_accounts: u64,
    pub lp_cooldown_time: u64,
    pub liquidation_margin_buffer_ratio: u32,
    pub settlement_duration: u16,
    pub number_of_markets: u16,
    pub number_of_spot_markets: u16,
    pub signer_nonce: u8,
    pub min_perp_auction_duration: u8,
    pub default_market_order_time_in_force: u8,
    pub default_spot_auction_duration: u8,
    pub exchange_status: u8,
    pub liquidation_duration: u8,
    pub initial_pct_to_liquidate: u16,
    pub max_number_of_sub_accounts: u16,
    pub max_initialize_user_fee: u16,
    pub feature_bit_flags: u8,
    pub padding: [u8; 9],
}
impl State {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "State");
}

pub struct DriftTestContext {
    pub admin: Keypair,
}

/// Setup Drift state in LiteSvm giving full control over state.
///
/// If anything is not set correctly for a subsequent test, either:
/// - IF applicable to all tests, mutate state with a set value here
/// - ELSE IF requires variable values for testing, add a argument
///     and mutate State set from the arg.
pub fn setup_drift_state(svm: &mut LiteSVM) -> DriftTestContext {
    let admin = Keypair::new();
    let mut state = State::default();
    // -- Update state variables
    state.admin = admin.pubkey();
    state.number_of_spot_markets = 2; // Allow at least 2 spot markets for testing

    let mut state_data = Vec::with_capacity(std::mem::size_of::<State>() + 8);
    state_data.extend_from_slice(&State::DISCRIMINATOR);
    state_data.extend_from_slice(&bytemuck::bytes_of(&state));

    let pubkey = derive_state_pda();
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

    DriftTestContext { admin }
}
