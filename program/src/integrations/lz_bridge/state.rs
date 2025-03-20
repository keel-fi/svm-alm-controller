use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct LzBridgeState {
    pub _padding: [u8;48]
}

