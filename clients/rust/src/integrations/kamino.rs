use solana_program::hash;
use solana_pubkey::Pubkey;

use crate::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID};

pub fn derive_vanilla_obligation_address(
    obligation_id: u8,
    authority: &Pubkey,
    market: &Pubkey,
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
        &KAMINO_LEND_PROGRAM_ID,
    );

    obligation_pda
}

pub fn derive_reserve_liquidity_supply(market: &Pubkey, reserve_liquidity_mint: &Pubkey) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_liq_supply",
            market.as_ref(),
            reserve_liquidity_mint.as_ref(),
        ],
        &KAMINO_LEND_PROGRAM_ID,
    );

    address
}

pub fn derive_reserve_collateral_mint(market: &Pubkey, reserve_liquidity_mint: &Pubkey) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_coll_mint",
            market.as_ref(),
            reserve_liquidity_mint.as_ref(),
        ],
        &KAMINO_LEND_PROGRAM_ID,
    );

    address
}

pub fn derive_reserve_collateral_supply(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_coll_supply",
            market.as_ref(),
            reserve_liquidity_mint.as_ref(),
        ],
        &KAMINO_LEND_PROGRAM_ID,
    );

    address
}

pub fn derive_market_authority_address(market: &Pubkey) -> (Pubkey, u8) {
    let (address, bump) =
        Pubkey::find_program_address(&[b"lma", market.as_ref()], &KAMINO_LEND_PROGRAM_ID);

    (address, bump)
}

pub fn derive_obligation_farm_address(reserve_farm: &Pubkey, obligation: &Pubkey) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[b"user", reserve_farm.as_ref(), &obligation.as_ref()],
        &KAMINO_FARMS_PROGRAM_ID,
    );

    address
}

pub fn derive_user_metadata_address(user: &Pubkey) -> Pubkey {
    let (address, _) =
        Pubkey::find_program_address(&[b"user_meta", &user.as_ref()], &KAMINO_LEND_PROGRAM_ID);

    address
}

pub fn derive_rewards_vault(farm_state: &Pubkey, rewards_vault_mint: &Pubkey) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[b"rvault", farm_state.as_ref(), rewards_vault_mint.as_ref()],
        &KAMINO_FARMS_PROGRAM_ID,
    );

    address
}

pub fn derive_rewards_treasury_vault(
    global_config: &Pubkey,
    rewards_vault_mint: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"tvault",
            global_config.as_ref(),
            rewards_vault_mint.as_ref(),
        ],
        &KAMINO_FARMS_PROGRAM_ID,
    );

    address
}

pub fn derive_farm_vaults_authority(farm_state: &Pubkey) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[b"authority", farm_state.as_ref()],
        &KAMINO_FARMS_PROGRAM_ID,
    );

    address
}


pub fn derive_anchor_discriminator(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);

    let mut sighash = [0_u8; 8];
    sighash.copy_from_slice(&hash::hash(preimage.as_bytes()).to_bytes()[..8]);

    sighash
}
