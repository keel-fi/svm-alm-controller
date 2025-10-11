use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct DriftConfig {
    pub sub_account_id: u16,
    pub _padding: [u8; 222],
}
