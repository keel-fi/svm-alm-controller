extern crate alloc;
use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankInstruction;

use crate::enums::{
    ControllerStatus, IntegrationStatus, IntegrationType, PermissionStatus, ReserveStatus,
};

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
    #[account(3, writable, name = "controller_authority")]
    #[account(4, writable, name = "permission")]
    #[account(5, name = "program_id")]
    #[account(6, name = "system_program")]
    InitializeController(InitializeControllerArgs),

    /// Manage an integration account
    #[account(0, writable, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, name = "program_id")]
    ManageController(ManageControllerArgs),

    /// Initialize or manage a permission account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, name = "controller_authority")]
    #[account(3, signer, name = "super_authority")]
    #[account(4, name = "super_permission")]
    #[account(5, name = "authority")]
    #[account(6, writable, name = "permission")]
    #[account(7, name = "program_id")]
    #[account(8, name = "system_program")]
    ManagePermission(ManagePermissionArgs),

    /// Initialize an reserve account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, name = "controller_authority")]
    #[account(3, signer, name = "authority")]
    #[account(4, name = "permission")]
    #[account(5, writable, name = "reserve")]
    #[account(6, name = "mint")]
    #[account(7, writable, name = "vault")]
    #[account(8, name = "token_program")]
    #[account(9, name = "associated_token_program")]
    #[account(10, name = "program_id")]
    #[account(11, name = "system_program")]
    InitializeReserve(InitializeReserveArgs),

    /// Manage and existing reserve account
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "reserve")]
    #[account(5, name = "program_id")]
    ManageReserve(ManageReserveArgs),

    /// Initialize an integration account
    #[account(0, writable, signer, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, name = "controller_authority")]
    #[account(3, signer, name = "authority")]
    #[account(4, name = "permission")]
    #[account(5, writable, name = "integration")]
    #[account(6, name = "lookup_table")]
    #[account(7, name = "program_id")]
    #[account(8, name = "system_program")]
    InitializeIntegration(InitializeIntegrationArgs),

    /// Manage an integration account
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    // NOTE: if there is no LUT, then the system_program ID should be used.
    #[account(5, name = "lookup_table")]
    #[account(6, name = "program_id")]
    ManageIntegration(ManageIntegrationArgs),

    // TOOD: Struct def does not match implementation. Has an extra `mint` account.
    /// SyncReserve
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, writable, name = "reserve")]
    #[account(3, name = "vault")]
    SyncReserve(SyncReserveArgs),

    /// SyncIntegration
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, writable, name = "integration")]
    #[account(3, name = "program_id")]
    Sync(SyncIntegrationArgs),

    /// Push
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, writable, name = "reserve_a")]
    #[account(6, writable, name = "reserve_b")]
    #[account(7, name = "program_id")]
    Push(PushArgs),

    /// Pull
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, writable, name = "reserve_a")]
    #[account(6, writable, name = "reserve_b")]
    #[account(7, name = "program_id")]
    Pull(PullArgs),

    /// InitializeOracle
    #[account(0, signer, writable, name = "payer")]
    #[account(1, signer, name = "authority")]
    #[account(2, name = "price_feed")]
    #[account(3, writable, name = "oracle")]
    #[account(4, name = "system_program")]
    InitializeOracle(InitializeOracleArgs),

    /// UpdateOracle
    #[account(0, signer, name = "authority")]
    #[account(1, name = "price_feed")]
    #[account(2, writable, name = "oracle")]
    #[account(3, optional, signer, name = "new_authority")]
    UpdateOracle(UpdateOracleArgs),

    /// RefreshOracle
    #[account(0, name = "price_feed")]
    #[account(1, writable, name = "oracle")]
    RefreshOracle(),

    /// Atomic swap borrow
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, writable, name = "reserve_a")]
    #[account(6, writable, name = "vault_a")]
    #[account(7, name = "mint_a")]
    #[account(8, writable, name = "reserve_b")]
    #[account(9, name = "vault_b")]
    #[account(10, writable, name = "recipient_token_account_a")]
    #[account(11, writable, name = "recipient_token_account_b")]
    #[account(12, name = "token_program_a")]
    #[account(13, name = "sysvar_instruction")]
    #[account(14, name = "program_id")]
    AtomicSwapBorrow(AtomicSwapBorrowArgs),

    /// Atomic swap repay
    #[account(0, signer, writable, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, writable, name = "reserve_a")]
    #[account(6, writable, name = "vault_a")]
    #[account(7, name = "mint_a")]
    #[account(8, writable, name = "reserve_b")]
    #[account(9, writable, name = "vault_b")]
    #[account(10, name = "mint_b")]
    #[account(11, name = "oracle")]
    #[account(12, writable, name = "payer_account_a")]
    #[account(13, writable, name = "payer_account_b")]
    #[account(14, name = "token_program_a")]
    #[account(15, name = "token_program_b")]
    AtomicSwapRepay,

    #[account(0, name = "controller")]
    #[account(1, writable, name = "integration")]
    #[account(2, writable, name = "sysvar_instruction")]
    ResetLzPushInFlight,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct EmitEventArgs {
    pub controller_id: [u8; 2],
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct InitializeControllerArgs {
    pub id: u16,
    pub status: ControllerStatus,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ManagePermissionArgs {
    pub status: PermissionStatus,
    pub can_manage_permissions: bool,
    pub can_invoke_external_transfer: bool,
    pub can_execute_swap: bool,
    pub can_reallocate: bool,
    pub can_freeze_controller: bool,
    pub can_unfreeze_controller: bool,
    pub can_manage_integrations: bool,
    pub can_suspend_permissions: bool,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct InitializeReserveArgs {
    pub status: ReserveStatus,
    pub rate_limit_slope: u64,
    pub rate_limit_max_outflow: u64,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ManageReserveArgs {
    pub status: Option<ReserveStatus>,
    pub rate_limit_slope: Option<u64>,
    pub rate_limit_max_outflow: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct InitializeIntegrationArgs {
    pub integration_type: IntegrationType,
    pub status: IntegrationStatus,
    pub description: [u8; 32],
    pub rate_limit_slope: u64,
    pub rate_limit_max_outflow: u64,
    pub inner_args: InitializeArgs,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ManageControllerArgs {
    pub status: ControllerStatus,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ManageIntegrationArgs {
    pub status: Option<IntegrationStatus>,
    pub description: Option<[u8; 32]>,
    pub rate_limit_slope: Option<u64>,
    pub rate_limit_max_outflow: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct InitializeOracleArgs {
    pub oracle_type: u8,
    pub nonce: Pubkey,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct UpdateOracleArgs {
    pub feed_args: Option<FeedArgs>,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct FeedArgs {
    pub oracle_type: u8,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum InitializeArgs {
    SplTokenExternal,
    SplTokenSwap,
    CctpBridge {
        desination_address: Pubkey,
        desination_domain: u32,
    },
    LzBridge {
        desination_address: Pubkey,
        destination_eid: u32,
    },
    AtomicSwap {
        max_slippage_bps: u16,
        max_staleness: u64,
        expiry_timestamp: i64,
        oracle_price_inverted: bool,
    },
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct SyncReserveArgs {}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct SyncIntegrationArgs {}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum PushArgs {
    SplTokenExternal {
        amount: u64,
    },
    SplTokenSwap {
        amount_a: u64,
        amount_b: u64,
        minimum_pool_token_amount: u64,
    },
    CctpBridge {
        amount: u64,
    },
    LzBridge {
        amount: u64,
    },
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum PullArgs {
    SplTokenExternal,
    SplTokenSwap {
        amount_a: u64,
        amount_b: u64,
        maximum_pool_token_amount: u64,
    },
    CctpBridge,
    LzBridge,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct AtomicSwapBorrowArgs {
    pub amount: u64,
}
