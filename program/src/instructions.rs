extern crate alloc;
use shank::ShankInstruction;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::enums::{ControllerStatus, PermissionStatus};

#[repr(C, u8)]
#[derive(Clone, Debug, PartialEq, ShankInstruction, BorshSerialize, BorshDeserialize)]
pub enum SvmAlmControllerInstruction {
    
    /// Initialize a Controller Account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, signer, name = "authority")]
    #[account(2, writable, name = "controller")]
    #[account(3, writable, name = "permission")]
    #[account(4, name = "system_program")]
    InitializeController(InitializeControllerArgs),

    /// Initialize or manage a permission account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, writable, name = "controller")]
    #[account(2, signer, name = "super_authority")]
    #[account(3, name = "super_permission")]
    #[account(4, name = "authority")]
    #[account(5, writable, name = "permission")]
    #[account(6, name = "system_program")]
    ManagePermission(ManagePermissionArgs),

     
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
}


