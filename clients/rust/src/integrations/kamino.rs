use solana_program::hash;
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
pub use uint_types::U256;
use core::ops::{Div, Mul};
use fixed::{FixedU128, types::extra::U60, traits::FromFixed};

pub fn derive_vanilla_obligation_address(
    obligation_id: u8,
    authority: &Pubkey,
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (obligation_pda, _) = Pubkey::find_program_address(
        &[
            // tag 0 for vanilla obligation
            &0_u8.to_le_bytes(),
            // id 0 as default
            &obligation_id.to_le_bytes(),
            // user
            authority.as_ref(),
            // kamino market
            market.as_ref(),
            // seed 1, for lending obligation is the token
            Pubkey::default().as_ref(),
            // seed 2, for lending obligation is the token
            Pubkey::default().as_ref(),
        ],
        kamino_program
    );

    obligation_pda
}

pub fn derive_anchor_discriminator(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);

    let mut sighash = [0_u8; 8];
    sighash.copy_from_slice(
        &hash::hash(preimage.as_bytes()).to_bytes()[..8]
    );

    sighash
}

/// Reduced version of `KaminoReserve`, with only the fields needed for liquidity value calculations
pub struct KaminoReserve {
    liquidity_available_amount: u64,
    liquidity_borrowed_amount_sf: u128,
    liquidity_accumulated_protocol_fees_sf: u128,
    liquidity_accumulated_referrer_fees_sf: u128,
    liquidity_pending_referrer_fees_sf: u128,
    collateral_mint_total_supply: u64,
}


impl KaminoReserve {

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

    pub fn try_deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 8 + RESERVE_SIZE {
            return Err(ProgramError::InvalidAccountData)
        }

        if data[..8] !=  derive_anchor_discriminator("account", "Reserve") {
            return Err(ProgramError::InvalidAccountData)
        }

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

        let collateral_mint_total_supply = u64::from_le_bytes(
            data[COLLATERAL_TOTAL_MINT_SUPPLY_OFFSET .. COLLATERAL_TOTAL_MINT_SUPPLY_OFFSET + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );

        Ok(Self { 
            liquidity_available_amount, 
            liquidity_borrowed_amount_sf, 
            liquidity_accumulated_protocol_fees_sf, 
            liquidity_accumulated_referrer_fees_sf, 
            liquidity_pending_referrer_fees_sf, 
            collateral_mint_total_supply 
        })
    }
}

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

/// Reduced version of `Obligation`, with only the fields needed for liquidity value calculations
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

impl Obligation {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 8 + OBLIGATION_SIZE {
            return Err(ProgramError::InvalidAccountData)
        }

        if data[..8] != derive_anchor_discriminator("account", "Obligation") {
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

pub const OBLIGATION_SIZE: usize = 3336;
pub const OBLIGATION_LENDING_MARKET_OFFSET: usize = 8 + 8 + 16;
pub const OWNER_OFFSET: usize = OBLIGATION_LENDING_MARKET_OFFSET + 32;
pub const DEPOSITS_OFFSET: usize = OWNER_OFFSET + 32;
pub const OBLIGATION_COLLATERAL_LEN: usize = 136;



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
