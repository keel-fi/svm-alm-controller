use core::ops::{Div, Mul};
use fixed::{traits::FromFixed, types::extra::U60, FixedU128};
use solana_sdk::program_error::ProgramError;
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

pub type Fraction = FixedU128<U60>;

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
