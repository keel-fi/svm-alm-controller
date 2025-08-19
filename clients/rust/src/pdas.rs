


// ------ kamino helpers ------

use solana_sdk::pubkey::Pubkey;

use crate::{
    constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, LUT_PROGRAM_ID}, 
    SVM_ALM_CONTROLLER_ID
};

pub fn derive_reserve_liquidity_supply(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_liq_supply",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        &KAMINO_LEND_PROGRAM_ID
    );

    address
}

pub fn derive_reserve_collateral_mint(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_coll_mint",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        &KAMINO_LEND_PROGRAM_ID
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
            reserve_liquidity_mint.as_ref()
        ], 
        &KAMINO_LEND_PROGRAM_ID
    );

    address
}

pub fn derive_market_authority_address(
    market: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"lma",
            market.as_ref(),
        ], 
        &KAMINO_LEND_PROGRAM_ID
    );

    address
}

pub fn derive_obligation_farm_address(
    reserve_farm: &Pubkey, 
    obligation: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"user",
            reserve_farm.as_ref(),
            &obligation.as_ref()
        ], 
        &KAMINO_FARMS_PROGRAM_ID
    );

    address
}

pub fn derive_user_metadata_address(
    user: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"user_meta",
            &user.as_ref()
        ], 
        &KAMINO_LEND_PROGRAM_ID
    );

    address
}

pub fn derive_lookup_table_address(
    authority_address: &Pubkey,
    recent_block_slot: u64,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            authority_address.as_ref(),
            &recent_block_slot.to_le_bytes()
        ], 
        &LUT_PROGRAM_ID
    );

    address
}

pub fn derive_rewards_vault(
    farm_state: &Pubkey,
    rewards_vault_mint: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"rvault",
            farm_state.as_ref(),
            rewards_vault_mint.as_ref()
        ], 
        &KAMINO_FARMS_PROGRAM_ID
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
            rewards_vault_mint.as_ref()
        ], 
        &KAMINO_FARMS_PROGRAM_ID
    );

    address
}

pub fn derive_farm_vaults_authority(
    farm_state: &Pubkey,
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"authority",
            farm_state.as_ref(),
        ], 
        &KAMINO_FARMS_PROGRAM_ID
    );

    address
}



// ------ SVM ALM Controller ------
pub fn derive_reserve_pda(controller_pda: &Pubkey, mint: &Pubkey) -> Pubkey {
    let (reserve_pda, _reserve_bump) = Pubkey::find_program_address(
        &[b"reserve", &controller_pda.to_bytes(), &mint.to_bytes()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    reserve_pda
}

pub fn derive_permission_pda(controller_pda: &Pubkey, authority: &Pubkey) -> Pubkey {
    let (permission_pda, _permission_bump) = Pubkey::find_program_address(
        &[
            b"permission",
            &controller_pda.to_bytes(),
            &authority.to_bytes(),
        ],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    permission_pda
}

pub fn derive_controller_authority_pda(controller_pda: &Pubkey) -> Pubkey {
    let (controller_authority_pda, _controller_authority_bump) = Pubkey::find_program_address(
        &[b"controller_authority", controller_pda.as_ref()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    controller_authority_pda
}

pub fn derive_integration_pda(controller_pda: &Pubkey, hash: &[u8; 32]) -> Pubkey {
    let (integration_pda, _integration_bump) = Pubkey::find_program_address(
        &[b"integration", &controller_pda.to_bytes(), &hash.as_ref()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    integration_pda
}