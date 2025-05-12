use pinocchio::pubkey::Pubkey;
use pinocchio_pubkey::pubkey;

pub const CONTROLLER_SEED: &[u8] = b"controller";
pub const PERMISSION_SEED: &[u8] = b"permission";
pub const RESERVE_SEED: &[u8] = b"reserve";
pub const INTEGRATION_SEED: &[u8] = b"integration";
pub const SPL_TOKEN_VAULT_SEED: &[u8] = b"vault";

pub const ADDRESS_LOOKUP_TABLE_PROGRAM_ID: Pubkey =
    pubkey!("AddressLookupTab1e1111111111111111111111111");

pub const SECONDS_PER_DAY: u64 = 86_400;
