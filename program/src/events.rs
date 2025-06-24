extern crate alloc;
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

use crate::state::{Controller, Integration, Permission, Reserve};

#[repr(u8)]
#[derive(Clone, Debug, PartialEq, ShankType, BorshSerialize, BorshDeserialize)]
pub enum SvmAlmControllerEvent {
    ControllerUpdate(ControllerUpdateEvent),
    PermissionUpdate(PermissionUpdateEvent),
    ReserveUpdate(ReserveUpdateEvent),
    IntegrationUpdate(IntegrationUpdateEvent),
    AccountingEvent(AccountingEvent),
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, ShankType)]
pub struct ControllerUpdateEvent {
    pub authority: Pubkey,
    pub controller: Pubkey,
    pub old_state: Option<Controller>,
    pub new_state: Option<Controller>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, ShankType)]
pub struct PermissionUpdateEvent {
    pub authority: Pubkey,
    pub controller: Pubkey,
    pub permission: Pubkey,
    pub old_state: Option<Permission>,
    pub new_state: Option<Permission>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, ShankType)]
pub struct ReserveUpdateEvent {
    pub authority: Pubkey,
    pub controller: Pubkey,
    pub reserve: Pubkey,
    pub old_state: Option<Reserve>,
    pub new_state: Option<Reserve>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, ShankType)]
pub struct IntegrationUpdateEvent {
    pub authority: Pubkey,
    pub controller: Pubkey,
    pub integration: Pubkey,
    pub old_state: Option<Integration>,
    pub new_state: Option<Integration>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, ShankType)]
pub struct AccountingEvent {
    pub controller: Pubkey,
    pub integration: Pubkey,
    pub mint: Pubkey,
    pub action: AccountingAction,
    pub before: u64,
    pub after: u64,
}

#[repr(u8)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, ShankType)]
pub enum AccountingAction {
    Sync,
    ExternalTransfer,
    Deposit,
    Withdrawal,
    BridgeSend,
}
