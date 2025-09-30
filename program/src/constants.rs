use pinocchio::pubkey::Pubkey;
use pinocchio_pubkey::pubkey;

pub const CONTROLLER_SEED: &[u8] = b"controller";
pub const CONTROLLER_AUTHORITY_SEED: &[u8] = b"controller_authority";
pub const PERMISSION_SEED: &[u8] = b"permission";
pub const RESERVE_SEED: &[u8] = b"reserve";
pub const INTEGRATION_SEED: &[u8] = b"integration";
pub const ORACLE_SEED: &[u8] = b"oracle";

pub const SECONDS_PER_DAY: u64 = 86_400;
pub const BPS_DENOMINATOR: u16 = 10_000;

pub const ATOMIC_SWAP_BORROW_IX_DISC: u8 = 15;
pub const ATOMIC_SWAP_REPAY_IX_DISC: u8 = 16;
pub const ATOMIC_SWAP_REPAY_INTEGRATION_IDX: u8 = 5;
pub const ATOMIC_SWAP_REPAY_PAYER_ACCOUNT_A_IDX: u8 = 13;
pub const ATOMIC_SWAP_REPAY_PAYER_ACCOUNT_B_IDX: u8 = 14;

// CCTP
pub const CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID: Pubkey =
    pubkey!("CCTPmbSD7gX1bxKPAmg77w8oFzNFpaQiQUWD43TKaecd");
pub const CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID: Pubkey =
    pubkey!("CCTPiPYPc6AsJuwueEnWgSgucamXDZwBd53dQ11YiKX3");
