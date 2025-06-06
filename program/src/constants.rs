use pinocchio::pubkey::Pubkey;
use pinocchio_pubkey::pubkey;

pub const CONTROLLER_SEED: &[u8] = b"controller";
pub const PERMISSION_SEED: &[u8] = b"permission";
pub const RESERVE_SEED: &[u8] = b"reserve";
pub const INTEGRATION_SEED: &[u8] = b"integration";
pub const SPL_TOKEN_VAULT_SEED: &[u8] = b"vault";
pub const ORACLE_SEED: &[u8] = b"oracle";

pub const ADDRESS_LOOKUP_TABLE_PROGRAM_ID: Pubkey =
    pubkey!("AddressLookupTab1e1111111111111111111111111");

pub const SECONDS_PER_DAY: u64 = 86_400;
pub const BPS_DENOMINATOR: u16 = 10_000;

pub const ATOMIC_SWAP_REPAY_IX_DISC: u8 = 15;
pub const ATOMIC_SWAP_REPAY_INTEGRATION_IDX: u8 = 4;

pub const LZ_OFT_SEND_IX_DISC: [u8; 8] = [102, 251, 20, 187, 65, 75, 12, 69]; // sha256("global:send")[0..8]
