
use borsh::{maybestd::{format, vec::Vec}, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo, cpi::{invoke, invoke_signed}, instruction::{AccountMeta, Instruction, Signer}, msg, program_error::ProgramError, pubkey::{find_program_address, Pubkey}, sysvars::clock::Slot
};
use solana_keccak_hasher::hash;

/// helper for finding an anchor instruction discriminator
pub fn anchor_sighash(namespace: &str, name: &str) -> [u8;8] {
    let preimage = format!("{}:{}", namespace, name);

    let mut sighash = [0u8; 8];
    sighash.copy_from_slice(
        &hash(preimage.as_bytes()).to_bytes()[..8]
    );
    sighash
}

// ------------ init obligation ------------

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct InitObligationArgs {
    pub tag: u8,
    pub id: u8
}

impl InitObligationArgs {
    pub const LEN: usize = 2;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let discriminator = anchor_sighash(
            "global", 
            "init_obligation"
        );

        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&discriminator);
        
        BorshSerialize::serialize(&self, &mut serialized).unwrap();
        
        Ok(serialized)
    }
}

pub fn derive_obligation_address(
    authority: &Pubkey,
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (obligation_pda, _) = find_program_address(
        &[
            // tag 0 for vanilla obligation
            &0_u8.to_le_bytes(),
            // id 0 as default
            &0_u8.to_le_bytes(),
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

pub fn initialize_obligation_cpi(
    tag: u8,
    id: u8,
    signer: Signer,
    obligation: &AccountInfo,
    obligation_owner: &AccountInfo,
    payer: &AccountInfo,
    market: &AccountInfo,
    owner_user_metadata: &AccountInfo,
    kamino_program: &Pubkey,
    rent: &AccountInfo,
    system_program: &AccountInfo
) -> Result<(), ProgramError> {

    let args_vec = InitObligationArgs { tag, id }
        .to_vec()
        .unwrap();

    let data = args_vec.as_slice();

    invoke_signed(
        &Instruction {
            program_id: kamino_program,
            data: &data,
            accounts: &[
                // obligation owner
                AccountMeta::readonly_signer(obligation_owner.key()),
                // fee payer
                AccountMeta::writable_signer(payer.key()),
                // obligation
                AccountMeta::writable(obligation.key()),
                // lending market
                AccountMeta::readonly(market.key()),
                // seed 1
                AccountMeta::readonly(&Pubkey::default()),
                // seed 2
                AccountMeta::readonly(&Pubkey::default()),
                // owner user metadata
                AccountMeta::readonly(owner_user_metadata.key()),
                // rent
                AccountMeta::readonly(rent.key()),
                // system program
                AccountMeta::readonly(system_program.key())
            ]
        }, 
        &[
            obligation_owner,
            payer,
            obligation,
            market,
            owner_user_metadata,
            rent,
            system_program
        ], 
        &[signer]
    )?;

    Ok(())
}

// ------------ init user metadata ------------

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct InitUserMetadataArgs<'a> {
    pub user_lookup_table: &'a Pubkey
}

impl<'a> InitUserMetadataArgs<'a> {
    pub const LEN: usize = 32;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let discriminator = anchor_sighash(
            "global", 
            "init_user_metadata"
        );

        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&discriminator);
        
        BorshSerialize::serialize(&self, &mut serialized).unwrap();
        
        Ok(serialized)
    }
}

pub fn derive_user_metadata_address(
    user: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = find_program_address(
        &[
            b"user_meta",
            &user.as_ref()
        ], 
        &kamino_program
    );

    address
}

pub fn initialize_user_metadata_cpi(
    signer: Signer,
    user: &AccountInfo,
    payer: &AccountInfo,
    user_metadata: &AccountInfo,
    user_lookup_table: &AccountInfo,
    // referrer_user_metadata: &AccountInfo, // TODO: confirm referrer user metadata
    kamino_program: &Pubkey,
    rent: &AccountInfo,
    system_program: &AccountInfo
) -> Result<(), ProgramError> {

    let args_vec = InitUserMetadataArgs {
        user_lookup_table: user_lookup_table.key()
    }.to_vec().unwrap();

    let data = args_vec.as_slice();

    invoke_signed(
        &Instruction { 
            program_id: kamino_program, 
            data: &data, 
            accounts: &[
                // owner
                AccountMeta::readonly_signer(user.key()),
                // fee payer
                AccountMeta::writable_signer(payer.key()),
                // user metadata
                AccountMeta::writable(user_metadata.key()),
                // referrer user metadata
                // AccountMeta::readonly(referrer_user_metadata.key()),
                // rent
                AccountMeta::readonly(rent.key()),
                // system program
                AccountMeta::readonly(system_program.key())
            ]
        }, 
        &[
            user,
            payer,
            user_metadata,
            // referrer_user_metadata,
            rent,
            system_program
        ], 
        &[signer]
    )?;

    Ok(())
}

// ------------ init user lookup table ------------

pub fn derive_lookup_table_address(
    authority_address: &Pubkey,
    recent_block_slot: Slot,
    lookup_table_program: &Pubkey
) -> (Pubkey, u8) {
    find_program_address(
        &[
            authority_address.as_ref(),
            &recent_block_slot.to_le_bytes()
        ], 
        lookup_table_program
    )
}

const CREATE_VARIANT_INDEX: u32 = 0;

/// Manual encoder: `[u32 variant | u64 recent_slot | u8 bump]`
pub fn encode_create_lookup_table(recent_slot: Slot, bump_seed: u8) -> [u8; 13] {
    let mut buf = [0u8; 13];
    buf[..4].copy_from_slice(&CREATE_VARIANT_INDEX.to_le_bytes());
    buf[4..12].copy_from_slice(&recent_slot.to_le_bytes());
    buf[12] = bump_seed;
    buf
}

pub fn initialize_user_lookup_table(
    signer: Signer,
    authority: &AccountInfo,
    payer: &AccountInfo,
    lookup_table: &AccountInfo,
    lookup_table_program: &Pubkey,
    system_program: &AccountInfo,
    recent_slot: Slot,
) -> Result<(), ProgramError> {
    let (lookup_table_address, bump_seed) = derive_lookup_table_address(
        authority.key(), 
        recent_slot, 
        lookup_table_program
    );

    if &lookup_table_address != lookup_table.key() {
        msg! {"Lookup table: Invalid lookup table"}
        return Err(ProgramError::InvalidSeeds)
    }

    let data = encode_create_lookup_table(recent_slot, bump_seed);

    invoke_signed(
        &Instruction { 
            program_id: lookup_table_program, 
            data: data.as_slice(), 
            accounts: &[
                // lut address
                AccountMeta::writable(lookup_table.key()),
                // lut authority
                AccountMeta::readonly_signer(authority.key()),
                // payer
                AccountMeta::writable(payer.key()),
                // system program
                AccountMeta::readonly(system_program.key())
            ] 
        }, 
        &[
            lookup_table,
            payer,
            authority,
            system_program
        ], 
        &[signer]
    )?;
    
    Ok(())
}


// ------------ init obligation farm ------------

pub fn derive_obligation_farm_address(
    reserve_farm: &Pubkey, 
    obligation: &Pubkey,
    kamino_farms_program: &Pubkey
) -> Pubkey {
    let (address, _) = find_program_address(
        &[
            b"user",
            reserve_farm.as_ref(),
            &obligation.as_ref()
        ], 
        &kamino_farms_program
    );

    address
}

pub fn derive_market_authority_address(
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = find_program_address(
        &[
            b"lma",
            market.as_ref(),
        ], 
        kamino_program
    );

    address
}

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct InitObligationFarmArgs {
    pub mode: u8
}

impl InitObligationFarmArgs {
    pub const LEN: usize = 1;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let discriminator = anchor_sighash(
            "global", 
            "init_obligation_farms_for_reserve"
        );

        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&discriminator);
        
        BorshSerialize::serialize(&self, &mut serialized).unwrap();
        
        Ok(serialized)
    }
}

pub fn initialize_obligation_farm_for_reserve_cpi(
    payer: &AccountInfo,
    owner: &AccountInfo,
    obligation: &AccountInfo,
    market_authority: &AccountInfo,
    reserve: &AccountInfo,
    // reserve.farm_collateral
    reserve_farm_state: &AccountInfo,
    obligation_farm: &AccountInfo,
    market: &AccountInfo,
    farms_program: &AccountInfo,
    rent: &AccountInfo,
    system_program: &AccountInfo,
    kamino_program: &Pubkey
) -> Result<(), ProgramError> {
    
    let args_vec = InitObligationFarmArgs {
        mode: 0 // TODO: verify this is correct mode
    }.to_vec().unwrap();

    let data = args_vec.as_slice();

    invoke(
        &Instruction { 
            program_id: kamino_program, 
            data: &data, 
            accounts: &[
                // payer
                AccountMeta::writable_signer(payer.key()),
                // owner
                AccountMeta::readonly(owner.key()),
                // obligation
                AccountMeta::writable(obligation.key()),
                // market authority
                AccountMeta::readonly(market_authority.key()),
                // reserve
                AccountMeta::writable(reserve.key()),
                // reserve farm state
                AccountMeta::writable(reserve_farm_state.key()),
                // obligation_farm
                AccountMeta::writable(obligation_farm.key()),
                // lending market
                AccountMeta::readonly(market.key()),
                // farms program
                AccountMeta::readonly(farms_program.key()),
                // rent
                AccountMeta::readonly(rent.key()),
                // system program
                AccountMeta::readonly(system_program.key())
            ] 
        },
        &[
            payer,
            owner,
            obligation,
            market_authority,
            reserve,
            reserve_farm_state,
            obligation_farm,
            market,
            farms_program,
            rent,
            system_program
        ]
    )?;

    Ok(())
}