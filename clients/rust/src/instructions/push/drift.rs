use solana_instruction::{AccountMeta, Instruction};
use solana_program::system_program;
use solana_pubkey::Pubkey;
use solana_sysvar::rent::ID as RENT_ID;

use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::PushBuilder, types::PushArgs},
    integrations::drift::{
        derive_spot_market_pda, derive_state_pda, derive_user_pda, derive_user_stats_pda
    },
};

/// Instruction generation for Drift "Push".
pub fn create_drift_push_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
    spot_market_index: u16,
    sub_account_id: u16,
    amount: u64,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);
    let permission_pda = derive_permission_pda(controller, authority);
    let drift_state_pda = derive_state_pda();
    let drift_user_stats_pda = derive_user_stats_pda(&controller_authority);
    let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
    let drift_spot_market_pda = derive_spot_market_pda(spot_market_index);

    let remaining_accounts = [
        AccountMeta {
            pubkey: drift_state_pda,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: drift_user_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: drift_user_stats_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: controller_authority,
            is_signer: true,
            is_writable: false,
        },
        AccountMeta {
            pubkey: drift_spot_market_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *authority, // user_token_account - for now using authority as placeholder
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: RENT_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: system_program::ID,
            is_signer: false,
            is_writable: false,
        },
    ];
    
    PushBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(permission_pda)
        .integration(*integration)
        .reserve_a(*reserve)
        .reserve_b(*reserve)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .push_args(PushArgs::Drift { amount })
        .add_remaining_accounts(&remaining_accounts)
        .instruction()
}
