use core::ops::{Div, Mul};

use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use fixed::{FixedU128, types::extra::U60, traits::FromFixed};
use crate::{
    integrations::kamino::{
        constants::{
            FARM_STATE_DISCRIMINATOR, 
            OBLIGATION_DISCRIMINATOR, 
            RESERVE_DISCRIMINATOR
        }, 
        initialize::InitializeKaminoAccounts
    }, processor::shared::is_account_closed
};


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

const RESERVE_SIZE: usize = 8616;
const RESERVE_LENDING_MARKET_OFFSET: usize = 8 + 8 + 16;
const FARM_COLLATERAL_OFFSET: usize = RESERVE_LENDING_MARKET_OFFSET + 32;
const FARM_DEBT_OFFSET: usize = FARM_COLLATERAL_OFFSET + 32;
const LIQUIDITY_MINT_OFFSET: usize = FARM_DEBT_OFFSET + 32;
const LIQUIDITY_AVAILABLE_AMOUNT_OFFSET: usize = LIQUIDITY_MINT_OFFSET + 32 + 32 + 32;
const LIQUIDITY_BORROWED_AMOUNT_OFFSET: usize = LIQUIDITY_AVAILABLE_AMOUNT_OFFSET + 8;
const LIQUIDITY_ACC_PROTOCOL_FEES_OFFSET: usize = LIQUIDITY_BORROWED_AMOUNT_OFFSET + 112;
const LIQUIDITY_ACC_REFERRER_FEES_OFFSET: usize = LIQUIDITY_ACC_PROTOCOL_FEES_OFFSET + 16;
const LIQUIDITY_PENDING_REFERRER_FEES: usize = LIQUIDITY_ACC_REFERRER_FEES_OFFSET + 16;
const COLLATERAL_MINT_OFFSET: usize = LIQUIDITY_PENDING_REFERRER_FEES + 2184;
const COLLATERAL_TOTAL_MINT_SUPPLY_OFFSET: usize = COLLATERAL_MINT_OFFSET + 32;

/// This is a slimmed down version of the `Reserve` state from `KLEND` program.
/// For more details, see: https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L60-L91
#[derive(Clone)]
pub struct KaminoReserve {
    pub lending_market: Pubkey,
    pub farm_collateral: Pubkey,
    pub farm_debt: Pubkey,
    pub liquidity_mint: Pubkey,
    pub liquidity_available_amount: u64,
    pub liquidity_borrowed_amount_sf: u128,
    pub liquidity_accumulated_protocol_fees_sf: u128,
    pub liquidity_accumulated_referrer_fees_sf: u128,
    pub liquidity_pending_referrer_fees_sf: u128,
    pub collateral_mint: Pubkey,
    pub collateral_mint_total_supply: u64
}


impl<'a> TryFrom<&'a [u8]> for KaminoReserve {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() < 8 + RESERVE_SIZE {
            return Err(ProgramError::InvalidAccountData)
        }

        if data[..8] != Self::DISCRIMINATOR {
            msg!("discriminator error");
            return Err(ProgramError::InvalidAccountData)
        }

        let lending_market = Pubkey::try_from(
            &data[RESERVE_LENDING_MARKET_OFFSET .. RESERVE_LENDING_MARKET_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let farm_collateral = Pubkey::try_from(
            &data[FARM_COLLATERAL_OFFSET .. FARM_COLLATERAL_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let farm_debt = Pubkey::try_from(
            &data[FARM_DEBT_OFFSET .. FARM_DEBT_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let liquidity_mint = Pubkey::try_from(
            &data[LIQUIDITY_MINT_OFFSET .. LIQUIDITY_MINT_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let liquidity_available_amount = u64::from_le_bytes(
            data[LIQUIDITY_AVAILABLE_AMOUNT_OFFSET .. LIQUIDITY_AVAILABLE_AMOUNT_OFFSET + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        let liquidity_borrowed_amount_sf = u128::from_le_bytes(
            data[LIQUIDITY_BORROWED_AMOUNT_OFFSET .. LIQUIDITY_BORROWED_AMOUNT_OFFSET + 16]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        let liquidity_accumulated_protocol_fees_sf = u128::from_le_bytes(
            data[LIQUIDITY_ACC_PROTOCOL_FEES_OFFSET .. LIQUIDITY_ACC_PROTOCOL_FEES_OFFSET + 16]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        let liquidity_accumulated_referrer_fees_sf = u128::from_le_bytes(
            data[LIQUIDITY_ACC_REFERRER_FEES_OFFSET .. LIQUIDITY_ACC_REFERRER_FEES_OFFSET + 16]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        let liquidity_pending_referrer_fees_sf = u128::from_le_bytes(
            data[LIQUIDITY_PENDING_REFERRER_FEES .. LIQUIDITY_PENDING_REFERRER_FEES + 16]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        let collateral_mint = Pubkey::try_from(
            &data[COLLATERAL_MINT_OFFSET .. COLLATERAL_MINT_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let collateral_mint_total_supply = u64::from_le_bytes(
            data[COLLATERAL_TOTAL_MINT_SUPPLY_OFFSET .. COLLATERAL_TOTAL_MINT_SUPPLY_OFFSET + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        Ok(Self { 
            lending_market, 
            farm_collateral, 
            farm_debt, 
            liquidity_mint,
            liquidity_available_amount,
            liquidity_borrowed_amount_sf,
            liquidity_accumulated_protocol_fees_sf,
            liquidity_accumulated_referrer_fees_sf,
            liquidity_pending_referrer_fees_sf,
            collateral_mint,
            collateral_mint_total_supply
        })
    }
}

impl KaminoReserve {
    pub const DISCRIMINATOR: [u8; 8] = RESERVE_DISCRIMINATOR;

    /// Verifies that:
    /// - the `Reserve` belongs to the market
    /// - the `Reserve` `liquidity_mint` matches `reserve_liquidity_mint`
    /// - the `Reserve` `farm_collateral` matches `reserve_farm_collateral` 
    /// - the `Reserve` `farm debt` matches `reserve_farm_debt`
    pub fn check_from_init_accounts(
        &self, 
        inner_ctx: &InitializeKaminoAccounts
    ) -> Result<(), ProgramError> {
        if &self.lending_market != inner_ctx.market.key() {
            msg! {"reserve: invalid reserve, does not belong to market"}
            return Err(ProgramError::InvalidAccountData)
        }

        if &self.liquidity_mint != inner_ctx.reserve_liquidity_mint.key() {
            msg! {"reserve: invalid reserve, liquidity mint does not match"}
            return Err(ProgramError::InvalidAccountData)
        }

        if &self.farm_collateral != inner_ctx.reserve_farm_collateral.key() {
            msg! {"reserve: farm collateral does not match reserve farm"}
            return Err(ProgramError::InvalidAccountData)
        }

        if &self.farm_debt != inner_ctx.reserve_farm_debt.key() {
            msg! {"reserve: farm debt does not match reserve farm"}
            return Err(ProgramError::InvalidAccountData)
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
        Fraction::from(self.liquidity_available_amount) 
            + Fraction::from_bits(self.liquidity_borrowed_amount_sf)
            - Fraction::from_bits(self.liquidity_accumulated_protocol_fees_sf)
            - Fraction::from_bits(self.liquidity_accumulated_referrer_fees_sf)
            - Fraction::from_bits(self.liquidity_pending_referrer_fees_sf)
    } 

    fn collateral_exchange_rate(&self) -> (u128, Fraction) {
        let mut total_liquidity = self.total_supply();
        let collateral_supply = {
            if self.collateral_mint_total_supply == 0 || total_liquidity == Fraction::ZERO {
                total_liquidity = Fraction::ONE;
                1
            } else {
                self.collateral_mint_total_supply
            }
        };

        (collateral_supply.into(), total_liquidity)
    }

    fn fraction_collateral_to_liquidity(&self, collateral_amount: Fraction) -> Fraction {
        let (collateral_supply, liquidity) = self.collateral_exchange_rate();

        (BigFraction::from(collateral_amount) * BigFraction::from(liquidity)
            / collateral_supply)
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

    let lp_amount = if is_account_closed(obligation) { 0 } else {
        // if it's not closed, then we read the state,
        // but its possible that the ObligationCollateral hasn't been created yet (first deposit)
        // in that case lp_amount is also 0
        let obligation_state = Obligation::try_from(
            obligation.try_borrow_data()?.as_ref()
        )?;
        // handles the case where no ObligationCollateral is found
        obligation_state.get_obligation_collateral_for_reserve(kamino_reserve.key())
            .map_or(0, |collateral| collateral.deposited_amount)
    };

    // avoids deserializing kamino_reserve if lp_amount is 0
    let liquidity_value = if lp_amount == 0 { 0 } else {
        let kamino_reserve_state = KaminoReserve::try_from(
            kamino_reserve.try_borrow_data()?.as_ref()
        )?;
        kamino_reserve_state.collateral_to_liquidity(lp_amount)
    };

    Ok((liquidity_value, lp_amount))
}


// --------- Obligation ----------

pub const OBLIGATION_SIZE: usize = 3336;
pub const OBLIGATION_LENDING_MARKET_OFFSET: usize = 8 + 8 + 16;
pub const OWNER_OFFSET: usize = OBLIGATION_LENDING_MARKET_OFFSET + 32;
pub const DEPOSITS_OFFSET: usize = OWNER_OFFSET + 32;
pub const OBLIGATION_COLLATERAL_LEN: usize = 136;

/// This is a slimmed down version of the `Obligation` state from `KLEND` program.
/// For more details, see: https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/obligation.rs#L26-L71
pub struct Obligation {
    pub lending_market: Pubkey,
    pub owner: Pubkey,
    pub deposits: [ObligationCollateral; 8]
}

#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct ObligationCollateral {
    pub reserve: Pubkey,
    pub deposited_amount: u64
}

impl<'a> TryFrom<&'a [u8]> for Obligation {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() < 8 + OBLIGATION_SIZE {
            return Err(ProgramError::InvalidAccountData)
        }

        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData)
        }

        let lending_market = Pubkey::try_from(
            &data[OBLIGATION_LENDING_MARKET_OFFSET .. OBLIGATION_LENDING_MARKET_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let owner = Pubkey::try_from(
            &data[OWNER_OFFSET .. OWNER_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let mut deposits = [ObligationCollateral::default(); 8];

        let mut current_offset = DEPOSITS_OFFSET;
        for slot in &mut deposits {
            let reserve = Pubkey::try_from(&data[current_offset .. current_offset + 32])
                .map_err(|_| ProgramError::InvalidAccountData)?;

            let deposited_amount = u64::from_le_bytes(
                data[current_offset + 32 .. current_offset + 32 + 8]
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?
            );

            *slot = ObligationCollateral {
                reserve,
                deposited_amount
            };

            current_offset += OBLIGATION_COLLATERAL_LEN;
        }

        Ok(Self { lending_market, owner, deposits })
    }
}

impl Obligation {
    pub const DISCRIMINATOR: [u8; 8] = OBLIGATION_DISCRIMINATOR;

    /// Verifies that:
    /// - the `Obligation` `owner` field matches `controller_authority`
    /// - the `Obligation` `lending_market` matches `market`
    pub fn check_data(
        &self,
        owner: &Pubkey,
        market: &Pubkey,
    ) -> Result<(), ProgramError> {

        if &self.owner != owner {
            msg! {"obligation: invalid obligation, owner is not controller_authority"}
            return Err(ProgramError::InvalidAccountData)
        }

        if &self.lending_market != market {
            msg! {"obligation: invalid obligation, belongs to another market"}
            return Err(ProgramError::InvalidAccountData)
        }

        Ok(())
    }

    pub fn is_deposits_full(&self) -> bool {
        self.deposits
            .iter()
            .find(|obligation_collateral| {
                obligation_collateral.reserve.eq(&Pubkey::default())
            })
            .is_none()
    }

    pub fn get_obligation_collateral_for_reserve(
        &self, 
        reserve: &Pubkey
    ) -> Option<&ObligationCollateral> {
        self.deposits
            .iter()
            .find(|obligation_collateral| {
                obligation_collateral.reserve.eq(reserve)
            })
    }
}

// --------- FarmState ---------

const FARMSTATE_SIZE: usize = 8328;
const GLOBAL_CONFIG_OFFSET: usize = 8 + 32;
const REWARD_INFOS_OFFSET: usize = GLOBAL_CONFIG_OFFSET + 32 + 120;
const REWARD_INFO_LEN: usize = 704;
const NUM_REWARD_TOKENS: usize = REWARD_INFOS_OFFSET + (REWARD_INFO_LEN * 10);

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct RewardInfo {
    pub token_mint: Pubkey,
    pub token_program: Pubkey,
    pub rewards_vault: Pubkey,
    pub rewards_available: u64,
}

/// This is a slimmed down version of the `FarmState` state from `KFARMS` program.
/// For more details, see: https://github.com/Kamino-Finance/kfarms/blob/master/programs/kfarms/src/state.rs#L70-L128
pub struct FarmState {
    pub global_config: Pubkey,
    pub rewards_info: [RewardInfo; 10],
    pub num_reward_tokens: u64
}

impl FarmState {
    const DISCRIMINATOR: [u8; 8] = FARM_STATE_DISCRIMINATOR;

    pub fn find_reward_index_and_rewards_available(
        &self, 
        reward_mint: &Pubkey,
        reward_token_program: &Pubkey
    ) -> Option<(u64, u64)> {
        self.rewards_info
            .iter()
            .enumerate()
            .find_map(|(index, reward_info)| {
                if &reward_info.token_mint == reward_mint
                    && &reward_info.token_program == reward_token_program
                {
                    Some((index as u64, reward_info.rewards_available))
                } else {
                    None
                }
            })
    }

    
}

impl<'a> TryFrom<&'a [u8]> for FarmState {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() < 8 + FARMSTATE_SIZE {
            return Err(ProgramError::InvalidAccountData)
        }

        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData)
        }

        let global_config = Pubkey::try_from(
            &data[GLOBAL_CONFIG_OFFSET .. GLOBAL_CONFIG_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let mut rewards_info = [RewardInfo::default(); 10]; 

        let mut current_offset = REWARD_INFOS_OFFSET;
        for slot in &mut rewards_info {
            let token_mint = Pubkey::try_from(&data[current_offset .. current_offset + 32])
                .map_err(|_| ProgramError::InvalidAccountData)?;

            let token_program = Pubkey::try_from(
                &data[current_offset + 32 + 8 .. current_offset + 32 + 8 + 32]
            )
            .map_err(|_| ProgramError::InvalidAccountData)?;

            let rewards_vault = Pubkey::try_from(
                &data[current_offset + 32 + 8 + 32 + 48 .. current_offset + 32 + 8 + 32 + 48 + 32]
            )
            .map_err(|_| ProgramError::InvalidAccountData)?;

            let rewards_available = u64::from_le_bytes(
                data[current_offset + 32 + 8 + 32 + 48 + 32 .. current_offset + 32 + 8 + 32 + 48 + 32 + 8]
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?
            );

            *slot = RewardInfo { 
                token_mint, 
                token_program, 
                rewards_vault,
                rewards_available
            };

            current_offset += REWARD_INFO_LEN;
        }

        let num_reward_tokens = u64::from_le_bytes(
            data[NUM_REWARD_TOKENS .. NUM_REWARD_TOKENS + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        Ok(Self { global_config, rewards_info, num_reward_tokens })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as bs64;
    use base64::Engine;
    use pinocchio_pubkey::pubkey;

    fn sf_u64(n: u64) -> u128 {
        Fraction::from_num(n).to_bits()
    }

    #[test]
    fn collateral_to_liquidity_works() {
        let base_reserve = KaminoReserve {
            lending_market: Pubkey::default(),
            farm_collateral: Pubkey::default(),
            farm_debt: Pubkey::default(),
            liquidity_mint: Pubkey::default(),
            liquidity_available_amount: 0,
            liquidity_borrowed_amount_sf: 0,
            liquidity_accumulated_protocol_fees_sf: 0,
            liquidity_accumulated_referrer_fees_sf: 0,
            liquidity_pending_referrer_fees_sf: 0,
            collateral_mint: Pubkey::default(),
            collateral_mint_total_supply: 0,
        };

        // 1:2 ratio -> 0.5x
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 1_000_000;
        reserve.collateral_mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 500);

        // 1:1 ratio -> 1x
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 1_000_000;
        reserve.collateral_mint_total_supply = 1_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 1_000);

        // borrowed liquidity adds up (1M + 3M) / 2M = 2x
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 1_000_000;
        reserve.liquidity_borrowed_amount_sf = sf_u64(3_000_000);
        reserve.collateral_mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 2_000);

        // fees reduce total (1M + 3M - 0.1M - 0.05M - 0.05M) / 2M = 1.9x
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 1_000_000;
        reserve.liquidity_borrowed_amount_sf = sf_u64(3_000_000);
        reserve.liquidity_accumulated_protocol_fees_sf = sf_u64(100_000);
        reserve.liquidity_accumulated_referrer_fees_sf = sf_u64(50_000);
        reserve.liquidity_pending_referrer_fees_sf = sf_u64(50_000);
        reserve.collateral_mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 1_900);

        // small ratio (0.05x)
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 500_000;
        reserve.collateral_mint_total_supply = 10_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 50);

        // rounding down (999_999 / 2_000_000 = 0.4999995 -> floor -> 499)
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 999_999;
        reserve.collateral_mint_total_supply = 2_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 499);

        // zero supply guard
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 0;
        reserve.collateral_mint_total_supply = 0;
        assert_eq!(reserve.collateral_to_liquidity(1), 1);

        // zero liquidity, nonzero collateral (guard path gives 1:1)
        let mut reserve = base_reserve.clone();
        reserve.liquidity_available_amount = 0;
        reserve.collateral_mint_total_supply = 1_000_000;
        assert_eq!(reserve.collateral_to_liquidity(1_000), 1_000);
    }

    #[test]
    fn reserve_try_from_works() {
        let lending_market = pubkey!("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF");
        let farm_collateral = pubkey!("955xWFhSDcDiUgUr4sBRtCpTLiMd4H5uZLAmgtP3R3sX");
        let liquidity_mint = pubkey!("So11111111111111111111111111111111111111112");
        let farm_debt = pubkey!("11111111111111111111111111111111");
        let liquidity_available_amount: u64 = 576438315861112;
        let liquidity_borrowed_amount_sf: u128 = 5235401463459533234106624313776750;
        let liquidity_accumulated_protocol_fees_sf: u128 = 11918939084660235266979009752828;
        let liquidity_accumulated_referrer_fees_sf: u128 = 0;
        let liquidity_pending_referrer_fees_sf: u128 = 0;
        let collateral_mint = pubkey!("2UywZrUdyqs5vDchy7fKQJKau2RVyuzBev2XKGPDSiX1");
        let collateral_mint_total_supply: u64 = 4684732222348610;

        let raw = bs64.decode(RAW_SOL_RESERVE_B64).expect("Invalid base 64 string");

        let reserve = KaminoReserve::try_from(raw.as_slice()).expect("Reserve try from error");
        
        assert_eq!(reserve.farm_collateral, farm_collateral);
        assert_eq!(reserve.lending_market, lending_market);
        assert_eq!(reserve.liquidity_mint, liquidity_mint);
        assert_eq!(reserve.farm_debt, farm_debt);
        assert_eq!(reserve.liquidity_available_amount, liquidity_available_amount);
        assert_eq!(reserve.liquidity_borrowed_amount_sf, liquidity_borrowed_amount_sf);
        assert_eq!(reserve.liquidity_accumulated_protocol_fees_sf, liquidity_accumulated_protocol_fees_sf);
        assert_eq!(reserve.liquidity_accumulated_referrer_fees_sf, liquidity_accumulated_referrer_fees_sf);
        assert_eq!(reserve.liquidity_pending_referrer_fees_sf, liquidity_pending_referrer_fees_sf);
        assert_eq!(reserve.collateral_mint, collateral_mint);
        assert_eq!(reserve.collateral_mint_total_supply, collateral_mint_total_supply);
    }

    #[test]
    fn obligation_try_from_works() {
            let lending_market = pubkey!("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF");
            let owner = pubkey!("4EiYqK5PCfLVyvL3WQRkifkPoVtaRHdTvYTzeLkydtHW");
            let collateral_reserves: [ObligationCollateral; 8] = [
                ObligationCollateral {
                    reserve: pubkey!("d4A2prbA2whesmvHaL88BH6Ewn5N4bTSU2Ze8P6Bc4Q"),
                    deposited_amount: 32990512301
                },
                ObligationCollateral::default(),
                ObligationCollateral::default(),
                ObligationCollateral::default(),
                ObligationCollateral::default(),
                ObligationCollateral::default(),
                ObligationCollateral::default(),
                ObligationCollateral::default(),
            ];

        let raw = bs64.decode(RAW_OBLIGATION_B64).expect("Invalid base 64 string");

        let obligation = Obligation::try_from(raw.as_slice()).expect("Obligation try from error");

        assert_eq!(obligation.lending_market, lending_market);
        assert_eq!(obligation.owner, owner);
        assert_eq!(obligation.deposits, collateral_reserves);

    }

    #[test]
    fn user_state_try_from_works() {
        let global_config = pubkey!("6UodrBjL2ZreDy7QdR4YV1oxqMBjVYSEyrFpctqqwGwL");
        let rewards_info = [
            RewardInfo { 
                token_mint: pubkey!("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"),
                token_program: pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
                rewards_vault: pubkey!("5JehzcYZjvqhijhhavULvgsM9BQfRyqVoztqhfJ7mdBn"),
                rewards_available: 0,
            },
            RewardInfo::default(),
            RewardInfo::default(),
            RewardInfo::default(),
            RewardInfo::default(),
            RewardInfo::default(),
            RewardInfo::default(),
            RewardInfo::default(),
            RewardInfo::default(),
            RewardInfo::default(),
        ];
        let num_reward_tokens = 1;

        let raw = bs64.decode(RAW_USDC_RESERVE_FARM_COLLATERAL).expect("Invalid base 64 string");

        let farm_state = FarmState::try_from(raw.as_slice()).expect("FarmState try from error");

        assert_eq!(farm_state.global_config, global_config);
        assert_eq!(farm_state.num_reward_tokens, num_reward_tokens);
        assert_eq!(farm_state.rewards_info, rewards_info);
    }


    const RAW_OBLIGATION_B64: &str = "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AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAkHgGrEJV4iT7AAAAAAAAAJB4BqxCVeIk+wAAAAAAAAAlIt3IygqipcoBAAAAAAAAYeOhcdS+TdjQAQAAAAAAAAD//////////wD///8AAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAASgAAAAAAZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";


    const RAW_SOL_RESERVE_B64: &str = "K/LMyhr3O38BAAAAAAAAAJSNbxUAAAAAAD8AAAAAAABmeujUWFWpdVBTSSyASh5w0QBYGag6K+J2mxkNDS3hEnfpbGtamjzJKMINHFtLE/NWFh0NdbeEouy5Rd68pomaAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGm4hX/quBhPtof2NGGMA12sQ53BrrO1WYoPAAAAAAAed+3UbWX4gjBzT0oRp5IViJAUh9cwa5QXiYw88h9sAXIirm5zkwpsVFaMQSd+TOnXPTwBptcqvuvWqxZ0W+JdZ4RA2BRAwCAG7a3Qj2WnHCZoF9DiACAQDnWkllaSsf9QsAAAAAAAAAZaGbaAAAAAAJAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADStRmNB9cWEgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD8bpH8QKEDjUFVK3CWAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABt324ddloZPZy+FGzut5rBy0he1fWzeROoz1hX7/AKkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABYGwH8gsbrGxHKGRCiq4JySBnfdAfDcVCvNsCVfCNa4Qh37N72kEABthVXfpzzMA/9YG4HpJbSlUfU2oMGKYFVVl2Uo/QUBHwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA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AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADn5lsHTDgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAC+wd1FaAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    
    const RAW_USDC_RESERVE_FARM_COLLATERAL: &str = "xmbYSj9Co76dYUjqNXOx2AFk19Mya5KUUJKDZFPPsCeMTQyecqQYGVFp2OF8VABdo+oDjIiuUd9TZa9Ao66dlvApdt5XYJQbAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAbd9uHXZaGT2cvhRs7reawctIXtX1s3kTqM9YV+/wCpAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAvAfFbmCtPT8Xc4LqxlSPuh/TLP2QygKz58+hhf3Oc5gFAAAAAAAAAAbd9uHXZaGT2cvhRs7reawctIXtX1s3kTqM9YV+/wCpAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP/RBtAmduarGe5Q0QR58X5yctVZf22agNAcaPfGAu+EAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAAAAAAAAAAAABwg7tlAAAAANRmV3/YhgAAKSYSujGBAgAWk+EQ5SwJPwwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAA//////////8AAAAAAAAAAP//////////AAAAAAAAAAD//////////wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAAAAAAAALQdAgAAAAAAkELPbZgLAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAANliyjMvBGmt/NZyM8eP4sNwFF1jhQcRvtDAhDj247MR/AAAAAAAAAB6KOn09rG0Wft6Z89dYlB+4mg26nT0qFjlqEA1CCNTigAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJBCz22YCwEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD//////////wAAAAAAAAAAnWFI6jVzsdgBZNfTMmuSlFCSg2RTz7AnjE0MnnKkGBkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJ4lquQ3JYHHlfqIONbgcLtQVW8GWDe5W1zDEcP7oMXwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
}