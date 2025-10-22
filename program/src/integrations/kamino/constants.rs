use crate::constants::anchor_discriminator;
use pinocchio::pubkey::Pubkey;
use pinocchio_pubkey::pubkey;

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
pub const FARMS_GLOBAL_CONFIG_DISCRIMINATOR: [u8; 8] =
    anchor_discriminator("account", "GlobalConfig");
