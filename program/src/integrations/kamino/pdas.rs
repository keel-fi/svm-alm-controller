use pinocchio::{
    program_error::ProgramError, 
    pubkey::{try_find_program_address, Pubkey}
};

use crate::integrations::kamino::constants::VANILLA_OBLIGATION_TAG;


pub fn derive_vanilla_obligation_address(
    obligation_id: u8,
    authority: &Pubkey,
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Result<Pubkey, ProgramError> {
    let (obligation_pda, _) = try_find_program_address(
        &[
            // tag 0 for vanilla obligation
            &VANILLA_OBLIGATION_TAG.to_le_bytes(),
            // id 0 as default
            &obligation_id.to_le_bytes(),
            // user
            authority.as_ref(),
            // kamino market
            market.as_ref(),
            // seed 1, pubkey default for vanilla obligations
            Pubkey::default().as_ref(),
            // seed 2, pubkey default for vanilla obligations
            Pubkey::default().as_ref(),
        ],
        kamino_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(obligation_pda)
}

pub fn derive_user_metadata_address(
    user: &Pubkey,
    kamino_program: &Pubkey
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"user_meta",
            &user.as_ref()
        ], 
        &kamino_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_obligation_farm_address(
    reserve_farm: &Pubkey, 
    obligation: &Pubkey,
    kamino_farms_program: &Pubkey
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"user",
            reserve_farm.as_ref(),
            &obligation.as_ref()
        ], 
        &kamino_farms_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_market_authority_address(
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"lma",
            market.as_ref(),
        ], 
        kamino_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_reserve_collateral_mint(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
    kamino_program: &Pubkey
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"reserve_coll_mint",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        kamino_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_reserve_collateral_supply(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
    kamino_program: &Pubkey
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"reserve_coll_supply",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        kamino_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_reserve_liquidity_supply(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
    kamino_program: &Pubkey
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"reserve_liq_supply",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        kamino_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_rewards_vault(
    farm_state: &Pubkey,
    rewards_vault_mint: &Pubkey,
    farms_program: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"rvault",
            farm_state.as_ref(),
            rewards_vault_mint.as_ref()
        ], 
        farms_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_rewards_treasury_vault(
    global_config: &Pubkey,
    rewards_vault_mint: &Pubkey,
    farms_program: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"tvault",
            global_config.as_ref(),
            rewards_vault_mint.as_ref()
        ], 
        farms_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_farm_vaults_authority(
    farm_state: &Pubkey,
    farms_program: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"authority",
            farm_state.as_ref(),
        ], 
        farms_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}