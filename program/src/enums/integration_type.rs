use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, Default, PartialEq, ShankType)]
#[repr(u8)]
pub enum IntegrationType {
    #[default]
    SplTokenExternal,
    CctpBridge,
    LzBridge,
    AtomicSwap,
}
