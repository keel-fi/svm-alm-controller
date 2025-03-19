extern crate alloc;
use alloc::vec::Vec;
use shank::ShankInstruction;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::enums::{ControllerStatus, IntegrationType, IntegrationStatus, PermissionStatus};

#[repr(C, u8)]
#[derive(Clone, Debug, PartialEq, ShankInstruction, BorshSerialize, BorshDeserialize)]
pub enum SvmAlmControllerInstruction {
    
    /// Emit event self CPI
    #[account(0, signer, name = "authority")]
    EmitEvent(EmitEventArgs),

    /// Initialize a Controller Account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, signer, name = "authority")]
    #[account(2, writable, name = "controller")]
    #[account(3, writable, name = "permission")]
    #[account(4, name = "system_program")]
    InitializeController(InitializeControllerArgs),

    /// Initialize or manage a permission account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, signer, name = "super_authority")]
    #[account(3, name = "super_permission")]
    #[account(4, name = "authority")]
    #[account(5, writable, name = "permission")]
    #[account(6, name = "system_program")]
    ManagePermission(ManagePermissionArgs),

    /// Initialize an integration account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, name = "lookup_table")]
    #[account(6, name = "system_program")]
    InializeIntegration(InitializeIntegrationArgs),

    /// Pull 
    #[account(0, name = "controller")]
    #[account(1, writable, name = "integration")]
    Sync(SyncArgs),

    /// Push 
    #[account(0, name = "controller")]
    #[account(1, signer, name = "authority")]
    #[account(2, name = "permission")]
    #[account(3, writable, name = "integration")]
    Push(PushArgs),

    /// Pull 
    #[account(0, name = "controller")]
    #[account(1, signer, name = "authority")]
    #[account(2, name = "permission")]
    #[account(3, writable, name = "integration")]
    Pull(PullArgs),
}


#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct EmitEventArgs {
    pub data: Vec<u8>
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct InitializeControllerArgs {
    pub id: u16,
    pub status: ControllerStatus 
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ManagePermissionArgs {
    pub status: PermissionStatus,
    pub can_manage_permissions: bool,
    pub can_invoke_external_transfer: bool,
    pub can_execute_swap: bool,
    pub can_reallocate: bool,
    pub can_freeze: bool,
    pub can_unfreeze: bool,
    pub can_manage_integrations: bool,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct InitializeIntegrationArgs {
    pub integration_type: IntegrationType,
    pub status: IntegrationStatus,
    pub description: [u8;32],
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct SyncArgs {}


#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum PushArgs {
    SplTokenVault,
    SplTokenExternal { amount: u64 },
    SplTokenSwap { amount_a: u64, amount_b: u64 },
}


#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum PullArgs {
    SplTokenVault,
    SplTokenExternal,
    SplTokenSwap { amount_a: u64, amount_b: u64 },
}


