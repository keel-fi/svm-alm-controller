use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;


#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct SplTokenExternalState {
    pub _padding: [u8;48]
}