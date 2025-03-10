use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct SplTokenVaultState {
    pub last_refresh_timestamp: i64,
    pub last_refresh_slot: u64,
    pub last_balance: u64,
    pub _padding: [u8;8]
}