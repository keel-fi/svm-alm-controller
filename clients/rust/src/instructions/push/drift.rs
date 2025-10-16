use bytemuck::{Pod, Zeroable};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::PushBuilder, types::PushArgs},
    integrations::drift::extract_spot_market_data,
    integrations::drift::{
        derive_spot_market_vault_pda, derive_state_pda, derive_user_pda, derive_user_stats_pda,
        DRIFT_PROGRAM_ID,
    },
};

/// Instruction generation for Drift "Push".
pub fn create_drift_push_instruction(
    controller: &Pubkey,
    super_authority: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
    reserve_vault: &Pubkey,
    user_token_account: &Pubkey,
    token_program: &Pubkey,
    spot_market_index: u16,
    sub_account_id: u16,
    amount: u64,
    reduce_only: bool,
    inner_remaining_accounts: &[AccountMeta],
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let controller_authority = derive_controller_authority_pda(controller);
    let permission_pda = derive_permission_pda(controller, super_authority);
    let drift_state_pda = derive_state_pda();
    let drift_user_stats_pda = derive_user_stats_pda(&controller_authority);
    let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
    let drift_spot_market_vault_pda = derive_spot_market_vault_pda(spot_market_index);

    let mut remaining_accounts = vec![
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
            pubkey: drift_spot_market_vault_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *user_token_account, // user_token_account - controller authority's ATA
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *token_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *reserve_vault, // reserve vault for balance sync
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: DRIFT_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
    ];

    remaining_accounts.extend(inner_remaining_accounts.iter().map(|account| AccountMeta {
        pubkey: account.pubkey,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
    }));

    let instruction = PushBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*super_authority)
        .permission(permission_pda)
        .integration(*integration)
        .reserve_a(*reserve)
        .reserve_b(*reserve)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .push_args(PushArgs::Drift {
            market_index: spot_market_index,
            amount,
            reduce_only,
        })
        .add_remaining_accounts(&remaining_accounts)
        .instruction();

    Ok(instruction)
}
