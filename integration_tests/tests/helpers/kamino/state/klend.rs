#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};
use litesvm::LiteSVM;
use solana_sdk::{program_error::ProgramError, pubkey::Pubkey};
use svm_alm_controller::constants::anchor_discriminator;

use crate::helpers::kamino::math_utils::{BigFraction, Fraction, FractionExtra};

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ReserveLiquidity {
    pub mint_pubkey: Pubkey,
    pub supply_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub available_amount: u64,
    pub borrowed_amount_sf: u128,
    pub market_price_sf: u128,
    pub market_price_last_updated_ts: u64,
    pub mint_decimals: u64,
    pub deposit_limit_crossed_timestamp: u64,
    pub borrow_limit_crossed_timestamp: u64,
    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    pub accumulated_protocol_fees_sf: u128,
    pub accumulated_referrer_fees_sf: u128,
    pub pending_referrer_fees_sf: u128,
    pub absolute_referral_rate_sf: u128,
    pub token_program: Pubkey,
    // padding expanded into 2 chunks to be Pod (length 51)
    pub _padding_1: [u64; 32],
    pub _padding_2: [u64; 19],
    pub _padding_3: [u128; 32],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ReserveCollateral {
    pub mint_pubkey: Pubkey,
    pub mint_total_supply: u64,
    pub supply_vault: Pubkey,
    pub _padding_1: [u128; 32],
    pub _padding_2: [u128; 32],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ReserveConfig {
    pub status: u8,
    pub asset_tier: u8,
    pub host_fixed_interest_rate_bps: u16,
    pub reserved_2: [u8; 9],
    pub protocol_order_execution_fee_pct: u8,
    pub protocol_take_rate_pct: u8,
    pub protocol_liquidation_fee_pct: u8,
    pub loan_to_value_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub min_liquidation_bonus_bps: u16,
    pub max_liquidation_bonus_bps: u16,
    pub bad_debt_liquidation_bonus_bps: u16,
    pub deleveraging_margin_call_period_secs: u64,
    pub deleveraging_threshold_decrease_bps_per_day: u64,
    pub fees: ReserveFees,
    pub borrow_rate_curve: BorrowRateCurve,
    pub borrow_factor_pct: u64,
    pub deposit_limit: u64,
    pub borrow_limit: u64,
    pub token_info: TokenInfo,
    pub deposit_withdrawal_cap: WithdrawalCaps,
    pub debt_withdrawal_cap: WithdrawalCaps,
    pub elevation_groups: [u8; 20],
    pub disable_usage_as_coll_outside_emode: u8,
    pub utilization_limit_block_borrowing_above_pct: u8,
    pub autodeleverage_enabled: u8,
    pub reserved_1: [u8; 1],
    pub borrow_limit_outside_elevation_group: u64,
    pub borrow_limit_against_this_collateral_in_elevation_group: [u64; 32],
    pub deleveraging_bonus_increase_bps_per_day: u64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct TokenInfo {
    pub name: [u8; 32],
    pub heuristic: PriceHeuristic,
    pub max_twap_divergence_bps: u64,
    pub max_age_price_seconds: u64,
    pub max_age_twap_seconds: u64,
    pub scope_configuration: ScopeConfiguration,
    pub switchboard_configuration: SwitchboardConfiguration,
    pub pyth_configuration: PythConfiguration,
    pub block_price_usage: u8,
    pub reserved: [u8; 7],
    pub _padding: [u64; 19],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct PythConfiguration {
    pub price: Pubkey,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct SwitchboardConfiguration {
    pub price_aggregator: Pubkey,
    pub twap_aggregator: Pubkey,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ScopeConfiguration {
    pub price_feed: Pubkey,
    pub price_chain: [u16; 4],
    pub twap_chain: [u16; 4],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct PriceHeuristic {
    pub lower: u64,
    pub upper: u64,
    pub exp: u64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    pub current_total: i64,
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct BorrowRateCurve {
    pub points: [CurvePoint; 11],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct CurvePoint {
    pub utilization_rate_bps: u32,
    pub borrow_rate_bps: u32,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ReserveFees {
    pub borrow_fee_sf: u64,
    pub flash_loan_fee_sf: u64,
    pub _padding: [u8; 8],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct KaminoReserve {
    pub version: u64,
    pub last_update: LastUpdate,
    pub lending_market: Pubkey,
    pub farm_collateral: Pubkey,
    pub farm_debt: Pubkey,
    pub liquidity: ReserveLiquidity,
    // padding expanded into 5 chunks to be Pod (length 150)
    pub _reserve_liquidity_padding_1: [u64; 30],
    pub _reserve_liquidity_padding_2: [u64; 30],
    pub _reserve_liquidity_padding_3: [u64; 30],
    pub _reserve_liquidity_padding_4: [u64; 30],
    pub _reserve_liquidity_padding_5: [u64; 30],
    pub collateral: ReserveCollateral,
    // padding expanded into 5 chunks to be Pod (length 150)
    pub _reserve_collateral_padding_1: [u64; 30],
    pub _reserve_collateral_padding_2: [u64; 30],
    pub _reserve_collateral_padding_3: [u64; 30],
    pub _reserve_collateral_padding_4: [u64; 30],
    pub _reserve_collateral_padding_5: [u64; 30],
    pub config: ReserveConfig,
    // padding expanded into 4 chunks to be Pod (length 116)
    pub _config_padding_1: [u64; 32],
    pub _config_padding_2: [u64; 32],
    pub _config_padding_3: [u64; 32],
    pub _config_padding_4: [u64; 20],
    pub borrowed_amount_outside_elevation_group: u64,
    pub borrowed_amounts_against_this_reserve_in_elevation_groups: [u64; 32],
    // padding expanded into 7 chunks to be Pod (length 207)
    pub _padding_1: [u64; 32],
    pub _padding_2: [u64; 32],
    pub _padding_3: [u64; 32],
    pub _padding_4: [u64; 32],
    pub _padding_5: [u64; 32],
    pub _padding_6: [u64; 32],
    pub _padding_7: [u64; 15],
}

impl KaminoReserve {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "Reserve");

    pub fn try_from(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn has_collateral_farm(&self) -> bool {
        self.farm_collateral != Pubkey::default()
    }

    pub fn has_debt_farm(&self) -> bool {
        self.farm_debt != Pubkey::default()
    }

    fn total_supply(&self) -> Fraction {
        Fraction::from(self.liquidity.available_amount)
            + Fraction::from_bits(self.liquidity.borrowed_amount_sf)
            - Fraction::from_bits(self.liquidity.accumulated_protocol_fees_sf)
            - Fraction::from_bits(self.liquidity.accumulated_referrer_fees_sf)
            - Fraction::from_bits(self.liquidity.pending_referrer_fees_sf)
    }

    fn collateral_exchange_rate(&self) -> (u128, Fraction) {
        let mut total_liquidity = self.total_supply();
        let collateral_supply = {
            if self.collateral.mint_total_supply == 0 || total_liquidity == Fraction::ZERO {
                total_liquidity = Fraction::ONE;
                1
            } else {
                self.collateral.mint_total_supply
            }
        };

        (collateral_supply.into(), total_liquidity)
    }

    fn fraction_collateral_to_liquidity(&self, collateral_amount: Fraction) -> Fraction {
        let (collateral_supply, liquidity) = self.collateral_exchange_rate();

        (BigFraction::from(collateral_amount) * BigFraction::from(liquidity) / collateral_supply)
            .try_into()
            .expect("fraction_collateral_to_liquidity: liquidity_amount overflow")
    }

    pub fn collateral_to_liquidity(&self, collateral_amount: u64) -> u64 {
        self.fraction_collateral_to_liquidity(collateral_amount.into())
            .to_floor()
    }
}

pub fn get_liquidity_and_lp_amount(
    svm: &LiteSVM,
    kamino_reserve_pk: &Pubkey,
    obligation_pk: &Pubkey,
) -> Result<(u64, u64), Box<dyn std::error::Error>> {
    let obligation_acc = svm
        .get_account(obligation_pk)
        .expect("could not get obligation");

    let obligation_state = Obligation::try_from(&obligation_acc.data)?;

    // if the obligation is closed
    // (there has been a full withdrawal and it only had one ObligationCollateral slot used),
    // then the lp_amount is 0
    let is_obligation_closed = obligation_acc.lamports == 0;

    let lp_amount = if is_obligation_closed {
        0
    } else {
        // if it's not closed, then we read the state,
        // but its possible that the ObligationCollateral hasn't been created yet (first deposit)
        // in that case lp_amount is also 0

        // handles the case where no ObligationCollateral is found
        obligation_state
            .get_obligation_collateral_for_reserve(kamino_reserve_pk)
            .map_or(0, |collateral| collateral.deposited_amount)
    };

    // avoids deserializing kamino_reserve if lp_amount is 0
    let liquidity_value = if lp_amount == 0 {
        0
    } else {
        let kamino_reserve_acc = svm
            .get_account(kamino_reserve_pk)
            .expect("could not get kamino reserve");
        let kamino_reserve_state = KaminoReserve::try_from(&kamino_reserve_acc.data)?;
        kamino_reserve_state.collateral_to_liquidity(lp_amount)
    };

    Ok((liquidity_value, lp_amount))
}

// --------- Obligation ----------
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct LastUpdate {
    slot: u64,
    stale: u8,
    price_status: u8,

    placeholder: [u8; 6],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ObligationCollateral {
    pub deposit_reserve: Pubkey,
    pub deposited_amount: u64,
    pub market_value_sf: u128,
    pub borrowed_amount_against_this_collateral_in_elevation_group: u64,
    pub _padding: [u64; 9],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct BigFractionBytes {
    pub value: [u64; 4],
    pub _padding: [u64; 2],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ObligationLiquidity {
    pub borrow_reserve: Pubkey,
    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    pub _padding: u64,
    pub borrowed_amount_sf: u128,
    pub market_value_sf: u128,
    pub borrow_factor_adjusted_market_value_sf: u128,

    pub borrowed_amount_outside_elevation_groups: u64,

    pub _padding_2: [u64; 7],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ObligationOrder {
    pub condition_threshold_sf: u128,
    pub opportunity_parameter_sf: u128,
    pub min_execution_bonus_bps: u16,
    pub max_execution_bonus_bps: u16,
    pub condition_type: u8,
    pub opportunity_type: u8,
    pub _padding_1: [u8; 10],
    pub _padding_2: [u128; 5],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct Obligation {
    pub tag: u64,
    pub last_update: LastUpdate,
    pub lending_market: Pubkey,
    pub owner: Pubkey,
    pub deposits: [ObligationCollateral; 8],
    pub lowest_reserve_deposit_liquidation_ltv: u64,
    pub deposited_value_sf: u128,
    pub borrows: [ObligationLiquidity; 5],
    pub borrow_factor_adjusted_debt_value_sf: u128,
    pub borrowed_assets_market_value_sf: u128,
    pub allowed_borrow_value_sf: u128,
    pub unhealthy_borrow_value_sf: u128,
    pub deposits_asset_tiers: [u8; 8],
    pub borrows_asset_tiers: [u8; 5],
    pub elevation_group: u8,
    pub num_of_obsolete_deposit_reserves: u8,
    pub has_debt: u8,
    pub referrer: Pubkey,
    pub borrowing_disabled: u8,
    pub autodeleverage_target_ltv_pct: u8,
    pub lowest_reserve_deposit_max_ltv_pct: u8,
    pub num_of_obsolete_borrow_reserves: u8,
    pub reserved: [u8; 4],
    pub highest_borrow_factor_pct: u64,
    pub autodeleverage_margin_call_started_timestamp: u64,
    pub orders: [ObligationOrder; 2],
    // padding expanded into 3 chunks to be Pod (length 93)
    pub _padding_1: [u64; 32],
    pub _padding_2: [u64; 32],
    pub _padding_3: [u64; 29],
}

impl Obligation {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "Obligation");

    pub fn try_from(data: &[u8]) -> Result<&Self, ProgramError> {
        let discriminator = data.get(..8).ok_or(ProgramError::InvalidAccountData)?;

        if discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn is_deposits_full(&self) -> bool {
        self.deposits
            .iter()
            .find(|obligation_collateral| {
                obligation_collateral.deposit_reserve.eq(&Pubkey::default())
            })
            .is_none()
    }

    pub fn get_obligation_collateral_for_reserve(
        &self,
        reserve: &Pubkey,
    ) -> Option<&ObligationCollateral> {
        self.deposits
            .iter()
            .find(|obligation_collateral| obligation_collateral.deposit_reserve.eq(reserve))
    }
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct LendingMarket {
    pub version: u64,
    pub bump_seed: u64,
    pub lending_market_owner: Pubkey,
    pub lending_market_owner_cached: Pubkey,
    pub quote_currency: [u8; 32],
    pub referral_fee_bps: u16,
    pub emergency_mode: u8,
    pub autodeleverage_enabled: u8,
    pub borrow_disabled: u8,
    pub price_refresh_trigger_to_max_age_pct: u8,
    pub liquidation_max_debt_close_factor_pct: u8,
    pub insolvency_risk_unhealthy_ltv_pct: u8,
    pub min_full_liquidation_value_threshold: u64,
    pub max_liquidatable_debt_market_value_at_once: u64,
    pub reserved0: [u8; 8],
    pub global_allowed_borrow_value: u64,
    pub risk_council: Pubkey,
    pub reserved1: [u8; 8],
    pub elevation_groups: [ElevationGroup; 32],
    pub elevation_group_padding1: [u64; 30],
    pub elevation_group_padding2: [u64; 30],
    pub elevation_group_padding3: [u64; 30],
    pub min_net_value_in_obligation_sf: u128,
    pub min_value_skip_liquidation_ltv_checks: u64,
    pub name: [u8; 32],
    pub min_value_skip_liquidation_bf_checks: u64,
    pub individual_autodeleverage_margin_call_period_secs: u64,
    pub min_initial_deposit_amount: u64,
    pub obligation_order_execution_enabled: u8,
    pub immutable: u8,
    pub obligation_order_creation_enabled: u8,
    pub padding2: [u8; 5],
    pub padding1: [u64; 30],
    pub padding3: [u64; 30],
    pub padding4: [u64; 30],
    pub padding5: [u64; 30],
    pub padding6: [u64; 30],
    pub padding7: [u64; 19],
}

impl LendingMarket {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "LendingMarket");

    pub fn try_from(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct ElevationGroup {
    pub max_liquidation_bonus_bps: u16,
    pub id: u8,
    pub ltv_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub allow_new_loans: u8,
    pub max_reserves_as_collateral: u8,
    pub padding_0: u8,
    pub debt_reserve: Pubkey,
    pub padding_1: [u64; 4],
}
