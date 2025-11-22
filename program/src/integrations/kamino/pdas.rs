use pinocchio::{
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
};

use crate::integrations::kamino::constants::{
    KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, VANILLA_OBLIGATION_TAG,
};

pub fn derive_vanilla_obligation_address(
    obligation_id: u8,
    authority: &Pubkey,
    market: &Pubkey,
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
        &KAMINO_LEND_PROGRAM_ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;

    Ok(obligation_pda)
}

pub fn derive_user_metadata_address(user: &Pubkey) -> Result<Pubkey, ProgramError> {
    let (address, _) =
        try_find_program_address(&[b"user_meta", &user.as_ref()], &KAMINO_LEND_PROGRAM_ID)
            .ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_obligation_farm_address(
    reserve_farm: &Pubkey,
    obligation: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[b"user", reserve_farm.as_ref(), &obligation.as_ref()],
        &KAMINO_FARMS_PROGRAM_ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_market_authority_address(market: &Pubkey) -> Result<Pubkey, ProgramError> {
    let (address, _) =
        try_find_program_address(&[b"lma", market.as_ref()], &KAMINO_LEND_PROGRAM_ID)
            .ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_rewards_vault(
    farm_state: &Pubkey,
    rewards_vault_mint: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[b"rvault", farm_state.as_ref(), rewards_vault_mint.as_ref()],
        &KAMINO_FARMS_PROGRAM_ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_rewards_treasury_vault(
    global_config: &Pubkey,
    rewards_vault_mint: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[
            b"tvault",
            global_config.as_ref(),
            rewards_vault_mint.as_ref(),
        ],
        &KAMINO_FARMS_PROGRAM_ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

pub fn derive_farm_vaults_authority(farm_state: &Pubkey) -> Result<Pubkey, ProgramError> {
    let (address, _) = try_find_program_address(
        &[b"authority", farm_state.as_ref()],
        &KAMINO_FARMS_PROGRAM_ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}
