use pinocchio::pubkey::Pubkey;
use pinocchio_pubkey::pubkey;
use sha2_const_stable::Sha256;

pub const CONTROLLER_SEED: &[u8] = b"controller";
pub const CONTROLLER_AUTHORITY_SEED: &[u8] = b"controller_authority";
pub const PERMISSION_SEED: &[u8] = b"permission";
pub const RESERVE_SEED: &[u8] = b"reserve";
pub const INTEGRATION_SEED: &[u8] = b"integration";
pub const SPL_TOKEN_VAULT_SEED: &[u8] = b"vault";
pub const ORACLE_SEED: &[u8] = b"oracle";

pub const ADDRESS_LOOKUP_TABLE_PROGRAM_ID: Pubkey =
    pubkey!("AddressLookupTab1e1111111111111111111111111");

pub const SECONDS_PER_DAY: u64 = 86_400;
pub const BPS_DENOMINATOR: u16 = 10_000;

pub const ATOMIC_SWAP_REPAY_IX_DISC: u8 = 16;
pub const ATOMIC_SWAP_REPAY_INTEGRATION_IDX: u8 = 4;

/// compute the first 8 bytes of SHA256(namespace:name) in a `const fn`.
pub const fn anchor_discriminator(namespace: &str, name: &str) -> [u8; 8] {
    let hash = Sha256::new()
        .update(namespace.as_bytes())
        .update(b":")
        .update(name.as_bytes())
        .finalize();

    // return the first 8 bytes as the discriminator
    [
        hash[0], hash[1], hash[2], hash[3],
        hash[4], hash[5], hash[6], hash[7],
    ]
}