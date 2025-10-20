use core::ops::{Div, Mul};

use crate::{
    integrations::kamino::{
        constants::{
            FARMS_GLOBAL_CONFIG_DISCRIMINATOR, FARM_STATE_DISCRIMINATOR, OBLIGATION_DISCRIMINATOR,
            RESERVE_DISCRIMINATOR, USER_FARM_STATE_DISCRIMINATOR,
        },
        initialize::InitializeKaminoAccounts,
    },
    processor::shared::is_account_closed,
};
use bytemuck::{Pod, Zeroable};
use fixed::{traits::FromFixed, types::extra::U60, FixedU128};
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};

// --------- from KLEND program ---------

pub use uint_types::U256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct BigFraction(pub U256);

impl<T> From<T> for BigFraction
where
    T: Into<Fraction>,
{
    fn from(fraction: T) -> Self {
        let fraction: Fraction = fraction.into();
        let repr_fraction = fraction.to_bits();
        Self(U256::from(repr_fraction))
    }
}

impl TryFrom<BigFraction> for Fraction {
    type Error = ProgramError;

    fn try_from(value: BigFraction) -> Result<Self, Self::Error> {
        let repr_faction: u128 = value
            .0
            .try_into()
            .map_err(|_| ProgramError::ArithmeticOverflow)?;
        Ok(Fraction::from_bits(repr_faction))
    }
}

impl Mul for BigFraction {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let extra_scaled = self.0 * rhs.0;
        let res = extra_scaled >> Fraction::FRAC_NBITS;
        Self(res)
    }
}

impl<T> Div<T> for BigFraction
where
    T: Into<U256>,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        let rhs: U256 = rhs.into();
        Self(self.0 / rhs)
    }
}

type Fraction = FixedU128<U60>;

pub trait FractionExtra {
    fn to_floor<Dst: FromFixed>(&self) -> Dst;
}

impl FractionExtra for Fraction {
    #[inline]
    fn to_floor<Dst: FromFixed>(&self) -> Dst {
        self.floor().to_num()
    }
}

#[allow(clippy::assign_op_pattern)]
#[allow(clippy::reversed_empty_ranges)]
mod uint_types {
    use uint::construct_uint;
    construct_uint! {
               pub struct U256(4);
    }
}

// --------- Reserve ----------
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
    pub const DISCRIMINATOR: [u8; 8] = RESERVE_DISCRIMINATOR;

    pub fn load_checked(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }

        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    /// Verifies that:
    /// - the `Reserve` belongs to the market
    /// - the `Reserve` `liquidity_mint` matches `reserve_liquidity_mint`
    /// - the `Reserve` `farm_collateral` matches `reserve_farm_collateral`
    /// - the `Reserve` `farm debt` matches `reserve_farm_debt`
    pub fn check_from_init_accounts(
        &self,
        inner_ctx: &InitializeKaminoAccounts,
    ) -> Result<(), ProgramError> {
        if &self.lending_market != inner_ctx.market.key() {
            msg! {"reserve: invalid reserve, does not belong to market"}
            return Err(ProgramError::InvalidAccountData);
        }

        if &self.liquidity.mint_pubkey != inner_ctx.reserve_liquidity_mint.key() {
            msg! {"reserve: invalid reserve, liquidity mint does not match"}
            return Err(ProgramError::InvalidAccountData);
        }

        if &self.farm_collateral != inner_ctx.reserve_farm_collateral.key() {
            msg! {"reserve: farm collateral does not match reserve farm"}
            return Err(ProgramError::InvalidAccountData);
        }

        if &self.farm_debt != inner_ctx.reserve_farm_debt.key() {
            msg! {"reserve: farm debt does not match reserve farm"}
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
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

/// This function gets the `liquidity_value` based on the `lp_amount` held.
/// It handles the cases where:
///     - The `Obligation` has been closed (full withdrawal).
///     - The `ObligationCollateral` doesn't exist yet (first deposit or full withdrawal).
///
/// Returns (`liquidity_value`, `lp_amount`)
pub fn get_liquidity_and_lp_amount(
    kamino_reserve: &AccountInfo,
    obligation: &AccountInfo,
) -> Result<(u64, u64), ProgramError> {
    // if the obligation is closed
    // (there has been a full withdrawal and it only had one ObligationCollateral slot used),
    // then the lp_amount is 0

    let lp_amount = if is_account_closed(obligation) {
        0
    } else {
        // if it's not closed, then we read the state,
        // but its possible that the ObligationCollateral hasn't been created yet (first deposit)
        // in that case lp_amount is also 0
        let obligation_data = obligation.try_borrow_data()?;
        let obligation_state = Obligation::load_checked(&obligation_data)?;

        // handles the case where no ObligationCollateral is found
        obligation_state
            .get_obligation_collateral_for_reserve(kamino_reserve.key())
            .map_or(0, |collateral| collateral.deposited_amount)
    };

    // avoids deserializing kamino_reserve if lp_amount is 0
    let liquidity_value = if lp_amount == 0 {
        0
    } else {
        let kamino_reserve_data = kamino_reserve.try_borrow_data()?;
        let kamino_reserve_state = KaminoReserve::load_checked(&kamino_reserve_data)?;
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
    pub const DISCRIMINATOR: [u8; 8] = OBLIGATION_DISCRIMINATOR;

    pub fn load_checked(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }

        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    /// Verifies that:
    /// - the `Obligation` `owner` field matches `controller_authority`
    /// - the `Obligation` `lending_market` matches `market`
    pub fn check_data(
        &self,
        controller_authority: &Pubkey,
        market: &Pubkey,
    ) -> Result<(), ProgramError> {
        if &self.owner != controller_authority {
            msg! {"obligation: invalid obligation, owner is not controller_authority"}
            return Err(ProgramError::InvalidAccountData);
        }

        if &self.lending_market != market {
            msg! {"obligation: invalid obligation, belongs to another market"}
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
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

// --------- FarmState ---------

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct FarmTokenInfo {
    pub mint: Pubkey,
    pub decimals: u64,
    pub token_program: Pubkey,
    pub _padding: [u64; 6],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct RewardInfo {
    pub token: FarmTokenInfo,
    pub rewards_vault: Pubkey,
    pub rewards_available: u64,
    pub reward_schedule_curve: RewardScheduleCurve,
    pub min_claim_duration_seconds: u64,
    pub last_issuance_ts: u64,
    pub rewards_issued_unclaimed: u64,
    pub rewards_issued_cumulative: u64,
    pub reward_per_share_scaled: u128,
    pub placeholder_0: u64,
    pub reward_type: u8,
    pub rewards_per_second_decimals: u8,
    pub _padding_0: [u8; 6],
    pub _padding_1: [u64; 20],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct RewardScheduleCurve {
    pub points: [RewardPerTimeUnitPoint; 20],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct RewardPerTimeUnitPoint {
    pub ts_start: u64,
    pub reward_per_time_unit: u64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct FarmState {
    pub farm_admin: Pubkey,
    pub global_config: Pubkey,
    pub token: FarmTokenInfo,
    pub reward_infos: [RewardInfo; 10],
    pub num_reward_tokens: u64,
    pub num_users: u64,
    pub total_staked_amount: u64,
    pub farm_vault: Pubkey,
    pub farm_vaults_authority: Pubkey,
    pub farm_vaults_authority_bump: u64,
    pub delegate_authority: Pubkey,
    pub time_unit: u8,
    pub is_farm_frozen: u8,
    pub is_farm_delegated: u8,
    pub _padding_0: [u8; 5],
    pub withdraw_authority: Pubkey,
    pub deposit_warmup_period: u32,
    pub withdrawal_cooldown_period: u32,
    pub total_active_stake_scaled: u128,
    pub total_pending_stake_scaled: u128,
    pub total_pending_amount: u64,
    pub slashed_amount_current: u64,
    pub slashed_amount_cumulative: u64,
    pub slashed_amount_spill_address: Pubkey,
    pub locking_mode: u64,
    pub locking_start_timestamp: u64,
    pub locking_duration: u64,
    pub locking_early_withdrawal_penalty_bps: u64,
    pub deposit_cap_amount: u64,
    pub scope_prices: Pubkey,
    pub scope_oracle_price_id: u64,
    pub scope_oracle_max_age: u64,
    pub pending_farm_admin: Pubkey,
    pub strategy_id: Pubkey,
    pub delegated_rps_admin: Pubkey,
    pub vault_id: Pubkey,
    pub second_delegated_authority: Pubkey,
    // padding expanded into 3 chunks to be Pod (length 74)
    pub _padding_1: [u64; 32],
    pub _padding_2: [u64; 32],
    pub _padding_3: [u64; 10],
}

impl FarmState {
    const DISCRIMINATOR: [u8; 8] = FARM_STATE_DISCRIMINATOR;

    pub fn load_checked(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }

        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn find_reward_index_and_rewards_available(
        &self,
        reward_mint: &Pubkey,
        reward_token_program: &Pubkey,
    ) -> Option<(u64, u64)> {
        self.reward_infos
            .iter()
            .enumerate()
            .find_map(|(index, reward_info)| {
                if &reward_info.token.mint == reward_mint
                    && &reward_info.token.token_program == reward_token_program
                {
                    Some((index as u64, reward_info.rewards_available))
                } else {
                    None
                }
            })
    }
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct GlobalConfig {
    pub global_admin: Pubkey,
    pub treasury_fee_bps: u64,
    pub treasury_vaults_authority: Pubkey,
    pub treasury_vaults_authority_bump: u64,
    pub pending_global_admin: Pubkey,
    // padding expanded into 4 chunks to be Pod (length 126)
    pub _padding_1: [u128; 32],
    pub _padding_2: [u128; 32],
    pub _padding_3: [u128; 32],
    pub _padding_4: [u128; 30],
}

impl GlobalConfig {
    const DISCRIMINATOR: [u8; 8] = FARMS_GLOBAL_CONFIG_DISCRIMINATOR;

    /// Load GlobalConfig account and check discriminator
    pub fn load_checked(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }

        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct UserState {
    pub user_id: u64,
    pub farm_state: Pubkey,
    pub owner: Pubkey,
    pub is_farm_delegated: u8,
    pub _padding_0: [u8; 7],
    pub rewards_tally_scaled: [u128; 10],
    pub rewards_issued_unclaimed: [u64; 10],
    pub last_claim_ts: [u64; 10],
    pub active_stake_scaled: u128,
    pub pending_deposit_stake_scaled: u128,
    pub pending_deposit_stake_ts: u64,
    pub pending_withdrawal_unstake_scaled: u128,
    pub pending_withdrawal_unstake_ts: u64,
    pub bump: u64,
    pub delegatee: Pubkey,
    pub last_stake_ts: u64,
    // padding expanded into 2 chunks to be Pod (length 50)
    pub _padding_1: [u64; 32],
    pub _padding_2: [u64; 18],
}

impl UserState {
    const DISCRIMINATOR: [u8; 8] = USER_FARM_STATE_DISCRIMINATOR;

    /// Load GlobalConfig account and check discriminator
    pub fn load_checked(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }

        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn get_rewards(
        user_state: &AccountInfo,
        global_config: &AccountInfo,
        reward_index: usize,
    ) -> Result<u64, ProgramError> {
        let user_state_data = user_state.try_borrow_data()?;
        let user_state = Self::load_checked(&user_state_data)?;

        let reward = user_state.rewards_issued_unclaimed[reward_index];
        if reward == 0 {
            return Ok(0);
        }

        let global_config_data = global_config.try_borrow_data()?;
        let global_config_state = GlobalConfig::load_checked(&global_config_data)?;
        let reward_treasury =
            Self::u64_mul_div(reward, global_config_state.treasury_fee_bps, 10000)?;
        let reward_user = reward
            .checked_sub(reward_treasury)
            .ok_or_else(|| ProgramError::ArithmeticOverflow)?;

        Ok(reward_user)
    }

    fn u64_mul_div(a: u64, b: u64, c: u64) -> Result<u64, ProgramError> {
        let a: u128 = a.into();
        let b: u128 = b.into();
        let c: u128 = c.into();

        let numerator = a.checked_mul(b).ok_or(ProgramError::ArithmeticOverflow)?;

        let result = numerator
            .checked_div(c)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        result
            .try_into()
            .map_err(|_| ProgramError::ArithmeticOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sf_u64(n: u64) -> u128 {
        Fraction::from_num(n).to_bits()
    }

    #[test]
    fn collateral_to_liquidity_works() {
        let base_reserve = KaminoReserve::default();

        // 1:2 ratio -> 0.5x
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 1_000_000;
        reserve.collateral.mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 500);

        // 1:1 ratio -> 1x
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 1_000_000;
        reserve.collateral.mint_total_supply = 1_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 1_000);

        // borrowed liquidity adds up (1M + 3M) / 2M = 2x
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 1_000_000;
        reserve.liquidity.borrowed_amount_sf = sf_u64(3_000_000);
        reserve.collateral.mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 2_000);

        // fees reduce total (1M + 3M - 0.1M - 0.05M - 0.05M) / 2M = 1.9x
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 1_000_000;
        reserve.liquidity.borrowed_amount_sf = sf_u64(3_000_000);
        reserve.liquidity.accumulated_protocol_fees_sf = sf_u64(100_000);
        reserve.liquidity.accumulated_referrer_fees_sf = sf_u64(50_000);
        reserve.liquidity.pending_referrer_fees_sf = sf_u64(50_000);
        reserve.collateral.mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 1_900);

        // small ratio (0.05x)
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 500_000;
        reserve.collateral.mint_total_supply = 10_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 50);

        // rounding down (999_999 / 2_000_000 = 0.4999995 -> floor -> 499)
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 999_999;
        reserve.collateral.mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 499);

        // zero supply guard
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 0;
        reserve.collateral.mint_total_supply = 0;
        assert_eq!(reserve.collateral_to_liquidity(1), 1);

        // zero liquidity, nonzero collateral (guard path gives 1:1)
        let mut reserve = base_reserve.clone();
        reserve.liquidity.available_amount = 0;
        reserve.collateral.mint_total_supply = 1_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 1_000);
    }
}
