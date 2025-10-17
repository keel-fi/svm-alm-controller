use pinocchio::pubkey::Pubkey;
use pinocchio_pubkey::pubkey;
use crate::constants::anchor_discriminator;

pub const VANILLA_OBLIGATION_TAG: u8 = 0;
pub const OBLIGATION_FARM_COLLATERAL_MODE: u8 = 0;
pub const OBLIGATION_FARM_DEBT_MODE: u8 = 1;

pub const KAMINO_LEND_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const KAMINO_FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");

// Kamino State discriminators
pub const RESERVE_DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "Reserve");
pub const OBLIGATION_DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "Obligation");
pub const FARM_STATE_DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "FarmState");
pub const USER_FARM_STATE_DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "UserState");
pub const FARMS_GLOBAL_CONFIG_DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "GlobalConfig");

// Kamino Cpi discriminators
pub const INIT_OBLIGATION_DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "init_obligation");
pub const INIT_METADATA_DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "init_user_metadata");
pub const INIT_OBLIGATION_FARM_DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "init_obligation_farms_for_reserve");
pub const DEPOSIT_LIQUIDITY_V2_DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "deposit_reserve_liquidity_and_obligation_collateral_v2");
pub const WITHDRAW_OBLIGATION_V2_DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2");
pub const HARVEST_REWARD_DISCRIMINATOR: [u8; 8] = anchor_discriminator("global", "harvest_reward");