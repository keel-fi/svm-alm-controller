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
    #[account(3, name = "controller_authority")]
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
    #[account(6, name = "program_id")]
    #[account(7, name = "system_program")]
    InitializeIntegration(InitializeIntegrationArgs),

    /// Manage an integration account
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, name = "program_id")]
    ManageIntegration(ManageIntegrationArgs),

    /// SyncReserve
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, writable, name = "reserve")]
    #[account(3, name = "vault")]
    SyncReserve(SyncReserveArgs),

    /// SyncIntegration
    #[account(0, name = "controller")]
    #[account(1, writable, name = "controller_authority")]
    #[account(2, writable, signer, name = "payer")]
    #[account(3, writable, name = "integration")]
    #[account(4, writable, name = "reserve")]
    Sync(SyncIntegrationArgs),

    /// Push
    #[account(0, name = "controller")]
    #[account(1, writable, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, writable, name = "reserve_a")]
    #[account(6, name = "program_id")]
    Push(PushArgs),

    /// Pull
    #[account(0, name = "controller")]
    #[account(1, writable, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "permission")]
    #[account(4, writable, name = "integration")]
    #[account(5, writable, name = "reserve_a")]
    #[account(6, name = "program_id")]
    Pull(PullArgs),

    /// InitializeOracle
    #[account(0, signer, writable, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, name = "controller_authority")]
    #[account(3, signer, name = "authority")]
    #[account(4, name = "price_feed")]
    #[account(5, writable, name = "oracle")]
    #[account(6, name = "system_program")]
    InitializeOracle(InitializeOracleArgs),

    /// UpdateOracle
    #[account(0, name = "controller")]
    #[account(1, name = "controller_authority")]
    #[account(2, signer, name = "authority")]
    #[account(3, name = "price_feed")]
    #[account(4, writable, name = "oracle")]
    #[account(5, optional, signer, name = "new_authority")]
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
    #[account(0, signer, name = "payer")]
    #[account(1, name = "controller")]
    #[account(2, name = "controller_authority")]
    #[account(3, signer, name = "authority")]
    #[account(4, name = "permission")]
    #[account(5, writable, name = "integration")]
    #[account(6, writable, name = "reserve_a")]
    #[account(7, writable, name = "vault_a")]
    #[account(8, name = "mint_a")]
    #[account(9, writable, name = "reserve_b")]
    #[account(10, writable, name = "vault_b")]
    #[account(11, name = "mint_b")]
    #[account(12, name = "oracle")]
    #[account(13, writable, name = "payer_account_a")]
    #[account(14, writable, name = "payer_account_b")]
    #[account(15, name = "token_program_a")]
    #[account(16, name = "token_program_b")]
    AtomicSwapRepay,

    #[account(0, name = "controller")]
    #[account(1, writable, name = "integration")]
    #[account(2, name = "sysvar_instruction")]
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
    pub can_manage_reserves_and_integrations: bool,
    pub can_suspend_permissions: bool,
    pub can_liquidate: bool,
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
    pub permit_liquidation: bool,
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
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
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
    CctpBridge {
        destination_address: Pubkey,
        destination_domain: u32,
    },
    LzBridge {
        destination_address: Pubkey,
        destination_eid: u32,
    },
    AtomicSwap {
        max_slippage_bps: u16,
        max_staleness: u64,
        expiry_timestamp: i64,
        oracle_price_inverted: bool,
    },
    Drift {
        sub_account_id: u16,
        spot_market_index: u16,
    },
    KaminoIntegration {
        obligation_id: u8,
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
    CctpBridge {
        amount: u64,
    },
    LzBridge {
        amount: u64,
    },
    Drift {
        market_index: u16,
        amount: u64,
        reduce_only: bool,
    },
    Kamino {
        amount: u64,
    },
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum PullArgs {
    SplTokenExternal,
    CctpBridge,
    LzBridge,
    Kamino {
        amount: u64,
    },
    Drift {
        market_index: u16,
        amount: u64,
        reduce_only: bool,
    },
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct AtomicSwapBorrowArgs {
    pub amount: u64,
}
