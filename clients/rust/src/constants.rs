use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

pub const CONTROLLER_SEED: &[u8] = b"controller";
pub const CONTROLLER_AUTHORITY_SEED: &[u8] = b"controller_authority";
pub const PERMISSION_SEED: &[u8] = b"permission";
pub const RESERVE_SEED: &[u8] = b"reserve";
pub const INTEGRATION_SEED: &[u8] = b"integration";
pub const SPL_TOKEN_SWAP_LP_SEED: &[u8] = b"spl-swap-lp";
pub const SPL_TOKEN_VAULT_SEED: &[u8] = b"vault";
pub const ORACLE_SEED: &[u8] = b"oracle";

pub const SPL_TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

pub const KAMINO_LEND_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const KAMINO_FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
pub const LUT_PROGRAM_ID: Pubkey = pubkey!("AddressLookupTab1e1111111111111111111111111");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
