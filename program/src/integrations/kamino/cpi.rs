
use borsh::{maybestd::vec::Vec, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo, cpi::{invoke, invoke_signed}, 
    instruction::{AccountMeta, Instruction, Signer}, msg, 
    program_error::ProgramError, pubkey::{try_find_program_address, Pubkey}, 
    sysvars::clock::Slot,
};

use crate::{
    error::SvmAlmControllerErrors, 
    integrations::kamino::constants::{
        DEPOSIT_LIQUIDITY_V2_DISCRIMINATOR, 
        HARVEST_REWARD_DISCRIMINATOR, 
        INIT_METADATA_DISCRIMINATOR, 
        INIT_OBLIGATION_DISCRIMINATOR, 
        INIT_OBLIGATION_FARM_DISCRIMINATOR, 
        WITHDRAW_OBLIGATION_V2_DISCRIMINATOR
    }
};

// ------------ init obligation ------------

pub const VANILLA_OBLIGATION_TAG: u8 = 0;

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct InitObligationArgs {
    pub tag: u8,
    pub id: u8
}

impl InitObligationArgs {
    pub const LEN: usize = 2;
    pub const DISCRIMINATOR: [u8; 8] = INIT_OBLIGATION_DISCRIMINATOR;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {

        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&Self::DISCRIMINATOR);
        
        BorshSerialize::serialize(&self, &mut serialized)
            .map_err(|_| ProgramError::from(SvmAlmControllerErrors::SerializationFailed))?;
        
        Ok(serialized)
    }
}

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

pub fn initialize_obligation_cpi(
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

    let args_vec = InitObligationArgs { tag: VANILLA_OBLIGATION_TAG, id }
        .to_vec()?;

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
            system_program,
            system_program,
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
    pub const DISCRIMINATOR: [u8; 8] = INIT_METADATA_DISCRIMINATOR;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&Self::DISCRIMINATOR);
        
        BorshSerialize::serialize(&self, &mut serialized)
            .map_err(|_| ProgramError::from(SvmAlmControllerErrors::SerializationFailed))?;
        
        Ok(serialized)
    }
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

pub fn initialize_user_metadata_cpi(
    signer: Signer,
    user: &AccountInfo,
    payer: &AccountInfo,
    user_metadata: &AccountInfo,
    user_lookup_table: &AccountInfo,
    referrer_user_metadata: &AccountInfo,
    kamino_program: &Pubkey,
    rent: &AccountInfo,
    system_program: &AccountInfo
) -> Result<(), ProgramError> {

    let args_vec = InitUserMetadataArgs {
        user_lookup_table: user_lookup_table.key()
    }.to_vec()?;

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
                // referrer user metadata (OPTIONAL)
                AccountMeta::readonly(referrer_user_metadata.key()),
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
            referrer_user_metadata,
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
) -> Result<(Pubkey, u8), ProgramError> {
    let result = try_find_program_address(
        &[
            authority_address.as_ref(),
            &recent_block_slot.to_le_bytes()
        ], 
        lookup_table_program
    ).ok_or(ProgramError::InvalidSeeds)?;

    Ok(result)
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
    )?;

    if &lookup_table_address != lookup_table.key() {
        msg! {"Lookup table: Invalid lookup table"}
        return Err(SvmAlmControllerErrors::InvalidPda.into())
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
                AccountMeta::writable_signer(payer.key()),
                // system program
                AccountMeta::readonly(system_program.key())
            ] 
        }, 
        &[
            lookup_table,
            authority,
            payer,
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

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct InitObligationFarmArgs {
    /// Mode 0 for collateral farm and mode 1 for debt farm
    pub mode: u8 
}

impl InitObligationFarmArgs {
    pub const LEN: usize = 1;
    pub const DISCRIMINATOR: [u8; 8] = INIT_OBLIGATION_FARM_DISCRIMINATOR;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&Self::DISCRIMINATOR);
        
        BorshSerialize::serialize(&self, &mut serialized)
            .map_err(|_| ProgramError::from(SvmAlmControllerErrors::SerializationFailed))?;
        
        Ok(serialized)
    }
}

pub const OBLIGATION_FARM_COLLATERAL_MODE: u8 = 0;
pub const OBLIGATION_FARM_DEBT_MODE: u8 = 1;

pub fn initialize_obligation_farm_for_reserve_cpi(
    // mode 0 for collateral farm and mode 1 for debt farm
    mode: u8,
    payer: &AccountInfo,
    owner: &AccountInfo,
    obligation: &AccountInfo,
    market_authority: &AccountInfo,
    reserve: &AccountInfo,
    // this is either reserve.farm_collateral or reserve.farm_debt
    reserve_farm_state: &AccountInfo,
    obligation_farm: &AccountInfo,
    market: &AccountInfo,
    farms_program: &AccountInfo,
    rent: &AccountInfo,
    system_program: &AccountInfo,
    kamino_program: &Pubkey
) -> Result<(), ProgramError> {
    
    let args_vec = InitObligationFarmArgs {
        // 0 for ReserveFarmKind::Collateral, meaning reserve_farm_state == reserve.farm_collateral
        // 1 for ReserveFarmKind::Debt, meaning reserve_farm_state == reserve.farm_debt
        mode
    }.to_vec()?;

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


// ---------- RESERVE derives ----------

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

// ---------- deposit reserve liquidity and obligation collateral ----------

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct DepositLiquidityV2Args {
    pub liquidity_amount: u64
}

impl DepositLiquidityV2Args {
    pub const LEN: usize = 8;
    pub const DISCRIMINATOR: [u8; 8] = DEPOSIT_LIQUIDITY_V2_DISCRIMINATOR;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&Self::DISCRIMINATOR);
        
        BorshSerialize::serialize(&self, &mut serialized)
            .map_err(|_| ProgramError::from(SvmAlmControllerErrors::SerializationFailed))?;
        
        Ok(serialized)
    }
}

pub fn deposit_reserve_liquidity_v2_cpi(
    liquidity_amount: u64,
    signer: Signer,
    owner: &AccountInfo,
    obligation: &AccountInfo,
    market: &AccountInfo,
    market_authority: &AccountInfo,
    reserve: &AccountInfo,
    reserve_liquidity_mint: &AccountInfo,
    reserve_liquidity_supply: &AccountInfo,
    reserve_collateral_mint: &AccountInfo,
    reserve_collateral_supply: &AccountInfo,
    liquidity_source: &AccountInfo,
    collateral_token_program: &AccountInfo,
    liquidity_token_program: &AccountInfo,
    instruction_sysvar_account: &AccountInfo,
    obligation_farm_collateral: &AccountInfo,
    reserve_farm_collateral: &AccountInfo,
    farms_program: &AccountInfo,
    kamino_program: &AccountInfo,
) -> Result<(), ProgramError> {
    let args_vec = DepositLiquidityV2Args {
        liquidity_amount
    }.to_vec()?;

    let data = args_vec.as_slice();

    invoke_signed(
        &Instruction { 
            program_id: kamino_program.key(), 
            data: &data, 
            accounts: &[
                // owner
                AccountMeta::writable_signer(owner.key()),
                // obligation
                AccountMeta::writable(obligation.key()),
                // lending market
                AccountMeta::readonly(market.key()),
                // market authority
                AccountMeta::readonly(market_authority.key()),
                // reserve
                AccountMeta::writable(reserve.key()),
                // reserve_liquidity_mint
                AccountMeta::readonly(reserve_liquidity_mint.key()),
                // reserve_liquidity_supply
                AccountMeta::writable(reserve_liquidity_supply.key()),
                // reserve_collateral_mint
                AccountMeta::writable(reserve_collateral_mint.key()),
                // reserve_destination_deposit_collateral
                AccountMeta::writable(reserve_collateral_supply.key()),
                // user_source_liquidity
                AccountMeta::writable(liquidity_source.key()),
                // placeholder_user_destination_collateral OPTIONAL
                AccountMeta::readonly(kamino_program.key()),
                // collateral_token_program
                AccountMeta::readonly(collateral_token_program.key()),
                // liquidity_token_program
                AccountMeta::readonly(liquidity_token_program.key()),
                // instruction_sysvar_account
                AccountMeta::readonly(instruction_sysvar_account.key()),
                // obligation_farm_user_state
                AccountMeta::writable(obligation_farm_collateral.key()),
                // reserve_farm_state
                AccountMeta::writable(reserve_farm_collateral.key()),
                // farms_program
                AccountMeta::readonly(farms_program.key()),
            ]
        }, 
        &[
            owner,
            obligation,
            market,
            market_authority,
            reserve,
            reserve_liquidity_mint,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_collateral_supply,
            liquidity_source,
            kamino_program,
            collateral_token_program,
            liquidity_token_program,
            instruction_sysvar_account,
            obligation_farm_collateral,
            reserve_farm_collateral,
            farms_program
        ], 
        &[signer]
    )?;

    Ok(())
}


// ---------- withdraw obligation collateral and redeem reserve collateral ----------

#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]

pub struct WithdrawObligationV2Args {
    pub collateral_amount: u64
}

impl WithdrawObligationV2Args {
    pub const LEN: usize = 8;
    pub const DISCRIMINATOR: [u8; 8] = WITHDRAW_OBLIGATION_V2_DISCRIMINATOR;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&Self::DISCRIMINATOR);
        
        BorshSerialize::serialize(&self, &mut serialized)
            .map_err(|_| ProgramError::from(SvmAlmControllerErrors::SerializationFailed))?;
        
        Ok(serialized)
    }
}

pub fn withdraw_obligation_collateral_v2_cpi(
    collateral_amount: u64,
    signer: Signer,
    owner: &AccountInfo,
    obligation: &AccountInfo,
    market: &AccountInfo,
    market_authority: &AccountInfo,
    reserve: &AccountInfo,
    reserve_liquidity_mint: &AccountInfo,
    reserve_liquidity_supply: &AccountInfo,
    reserve_collateral_mint: &AccountInfo,
    reserve_collateral_supply: &AccountInfo,
    liquidity_destination: &AccountInfo,
    collateral_token_program: &AccountInfo,
    liquidity_token_program: &AccountInfo,
    instruction_sysvar_account: &AccountInfo,
    obligation_farm_collateral: &AccountInfo,
    reserve_farm_collateral: &AccountInfo,
    farms_program: &AccountInfo,
    kamino_program: &AccountInfo,
) -> Result<(), ProgramError> {
    let args_vec = WithdrawObligationV2Args {
        collateral_amount
    }.to_vec()?;

    let data = args_vec.as_slice();

    invoke_signed(
        &Instruction { 
            program_id: kamino_program.key(), 
            data: &data, 
            accounts: &[
                // owner
                AccountMeta::writable_signer(owner.key()),
                // obligation
                AccountMeta::writable(obligation.key()),
                // market
                AccountMeta::readonly(market.key()),
                // market authority
                AccountMeta::readonly(market_authority.key()),
                // reserve
                AccountMeta::writable(reserve.key()),
                // reserve liquidity mint
                AccountMeta::readonly(reserve_liquidity_mint.key()),
                // reserve collateral supply vault
                AccountMeta::writable(reserve_collateral_supply.key()),
                // reserve collateral mint
                AccountMeta::writable(reserve_collateral_mint.key()),
                // reserve liquidity supply vault
                AccountMeta::writable(reserve_liquidity_supply.key()),
                // user destination collateral
                AccountMeta::writable(liquidity_destination.key()),
                // placeholder_user_destination_collateral OPTIONAL
                AccountMeta::readonly(kamino_program.key()),
                // collateral token program
                AccountMeta::readonly(collateral_token_program.key()),
                // liquidity_token_program
                AccountMeta::readonly(liquidity_token_program.key()),
                // instruction sysvar account
                AccountMeta::readonly(instruction_sysvar_account.key()),
                // obligation_farm_user_state
                AccountMeta::writable(obligation_farm_collateral.key()),
                // reserve_farm_state
                AccountMeta::writable(reserve_farm_collateral.key()),
                // farms program
                AccountMeta::readonly(farms_program.key()),
            ]
        },
        &[
            owner,
            obligation,
            market,
            market_authority,
            reserve,
            reserve_liquidity_mint,
            reserve_collateral_supply,
            reserve_collateral_mint,
            reserve_liquidity_supply,
            liquidity_destination,
            kamino_program,
            collateral_token_program,
            liquidity_token_program,
            instruction_sysvar_account,
            obligation_farm_collateral,
            reserve_farm_collateral,
            farms_program
        ], 
        &[signer]
    )?;

    Ok(())
}


// -------- harvest farm rewards --------

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


#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct HarvestRewardArgs {
    pub reward_index: u64
}

impl HarvestRewardArgs {
    pub const LEN: usize = 8;
    pub const DISCRIMINATOR: [u8; 8] = HARVEST_REWARD_DISCRIMINATOR;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&Self::DISCRIMINATOR);
        
        BorshSerialize::serialize(&self, &mut serialized)
            .map_err(|_| ProgramError::from(SvmAlmControllerErrors::SerializationFailed))?;
        
        Ok(serialized)
    }
}

pub fn harvest_reward_cpi(
    reward_index: u64,
    signer: Signer,
    owner: &AccountInfo,
    user_state: &AccountInfo,
    farm_state: &AccountInfo,
    global_config: &AccountInfo,
    reward_mint: &AccountInfo,
    user_reward_ata: &AccountInfo,
    rewards_vault: &AccountInfo,
    rewards_treasury_vault: &AccountInfo,
    farm_vaults_authority: &AccountInfo,
    scope_prices: &AccountInfo,
    token_program: &AccountInfo,
    farms_program: &AccountInfo
) -> Result<(), ProgramError> {
    let args_vec = HarvestRewardArgs {
        reward_index
    }.to_vec()?;

    let data = args_vec.as_slice();

    invoke_signed(
        &Instruction { 
            program_id: farms_program.key(), 
            data: &data, 
            accounts: &[
                // owner
                AccountMeta::writable_signer(owner.key()),
                // user state
                AccountMeta::writable(user_state.key()),
                // farm_state
                AccountMeta::writable(farm_state.key()),
                // global_config
                AccountMeta::readonly(global_config.key()),
                // reward_mint
                AccountMeta::readonly(reward_mint.key()),
                // user reward ata
                AccountMeta::writable(user_reward_ata.key()),
                // rewards_vault
                AccountMeta::writable(rewards_vault.key()),
                // rewards_treasury_vault
                AccountMeta::writable(rewards_treasury_vault.key()),
                // farm_vaults_authority
                AccountMeta::readonly(farm_vaults_authority.key()),
                // scope_prices
                AccountMeta::readonly(scope_prices.key()),
                // token_progra
                AccountMeta::readonly(token_program.key()),
            ] 
        }, 
        &[
            owner,
            user_state,
            farm_state,
            global_config,
            reward_mint,
            user_reward_ata,
            rewards_vault,
            rewards_treasury_vault,
            farm_vaults_authority,
            scope_prices,
            token_program,
        ],
        &[signer]
    )?;

    Ok(())
}