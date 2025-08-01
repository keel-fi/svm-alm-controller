use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

use crate::integrations::utilization_market::kamino::cpi::anchor_sighash;

// --------- Reserve ----------

pub const RESERVE_SIZE: usize = 8616;
pub const RESERVE_LENDING_MARKET_OFFSET: usize = 8 + 8 + 16;
pub const FARM_COLLATERAL_OFFSET: usize = RESERVE_LENDING_MARKET_OFFSET + 32;
pub const LIQUIDITY_MINT_OFFSET: usize = FARM_COLLATERAL_OFFSET + 32 + 32;

pub struct Reserve {
    pub lending_market: Pubkey,
    pub farm_collateral: Pubkey,
    pub liquidity_mint: Pubkey,
}


impl<'a> TryFrom<&'a [u8]> for Reserve {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() < 8 + RESERVE_SIZE {
            return Err(ProgramError::InvalidAccountData)
        }

        let discriminator = anchor_sighash("account", "Reserve");

        if data[..8] != discriminator {
            return Err(ProgramError::InvalidAccountData)
        }

        let lending_market = Pubkey::try_from(
            &data[RESERVE_LENDING_MARKET_OFFSET .. RESERVE_LENDING_MARKET_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let farm_collateral = Pubkey::try_from(
            &data[FARM_COLLATERAL_OFFSET .. FARM_COLLATERAL_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let liquidity_mint = Pubkey::try_from(
            &data[LIQUIDITY_MINT_OFFSET .. LIQUIDITY_MINT_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        Ok(Self { lending_market, farm_collateral, liquidity_mint })
    }
}



// --------- Obligation ----------

pub const OBLIGATION_SIZE: usize = 3336;
pub const OBLIGATION_LENDING_MARKET_OFFSET: usize = 8 + 8 + 16;
pub const OWNER_OFFSET: usize = OBLIGATION_LENDING_MARKET_OFFSET + 32;
pub const DEPOSITS_OFFSET: usize = OWNER_OFFSET + 32;
pub const OBLIGATION_COLLATERAL_LEN: usize = 136;

pub struct Obligation {
    pub lending_market: Pubkey,
    pub owner: Pubkey,
    pub collateral_reserves: [Pubkey; 8]
}

impl<'a> TryFrom<&'a [u8]> for Obligation {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() < 8 + OBLIGATION_SIZE {
            return Err(ProgramError::InvalidAccountData)
        }

        let discriminator = anchor_sighash("account", "Obligation");

        if data[..8] != discriminator {
            return Err(ProgramError::InvalidAccountData)
        }

        let lending_market = Pubkey::try_from(
            &data[OBLIGATION_LENDING_MARKET_OFFSET .. OBLIGATION_LENDING_MARKET_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let owner = Pubkey::try_from(
            &data[OWNER_OFFSET .. OWNER_OFFSET + 32]
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        let mut collateral_reserves = [Pubkey::default(); 8];

        let mut current_offset = DEPOSITS_OFFSET;
        for slot in &mut collateral_reserves {
            *slot = Pubkey::try_from(&data[current_offset .. current_offset + 32])
                .map_err(|_| ProgramError::InvalidAccountData)?;

            current_offset += OBLIGATION_COLLATERAL_LEN;
        }

        Ok(Self { lending_market, owner, collateral_reserves })
    }
}