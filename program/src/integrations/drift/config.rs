use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct DriftConfig {
    pub _padding: [u8; 224],
}
