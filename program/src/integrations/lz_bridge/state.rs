use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct LzBridgeState {
    /// Flag indicating that a LZ push instruction
    /// is underway. This is a safety mechanism to prevent
    /// multiple pushes from preceding an OFT Send.
    pub push_in_flight: bool,
    pub _padding: [u8; 47],
}
