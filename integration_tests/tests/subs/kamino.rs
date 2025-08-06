use solana_sdk::{
    pubkey::Pubkey,
};


pub fn derive_market_authority_address(
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"lma",
            market.as_ref(),
        ], 
        kamino_program
    );

    address
}

pub fn derive_user_metadata_address(
    user: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"user_meta",
            &user.as_ref()
        ], 
        &kamino_program
    );

    address
}

pub fn derive_lookup_table_address(
    authority_address: &Pubkey,
    recent_block_slot: u64,
    lookup_table_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            authority_address.as_ref(),
            &recent_block_slot.to_le_bytes()
        ], 
        lookup_table_program
    );

    address
}

pub fn derive_obligation_farm_address(
    reserve_farm: &Pubkey, 
    obligation: &Pubkey,
    kamino_farms_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"user",
            reserve_farm.as_ref(),
            &obligation.as_ref()
        ], 
        &kamino_farms_program
    );

    address
}

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