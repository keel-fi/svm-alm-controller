use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, Default, PartialEq, ShankType)]
#[repr(u8)]
pub enum ControllerStatus {
    /// All instructions interacting with the Controller should revert, except to change the status to Active.
    #[default]
    Frozen,
    /// Normal operations
    Active,
    /// Push or Pull instructions revert, but all other operations are valid
    PushPullFrozen,
}
