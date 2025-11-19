use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

use crate::{
    derive_controller_authority_pda, derive_permission_pda, derive_reserve_pda,
    generated::{instructions::PushBuilder, types::PushArgs},
};

pub fn create_psm_swap_push_instruction(
    controller: &Pubkey,
    super_authority: &Pubkey,
    mint: &Pubkey,
    integration: &Pubkey,
    token_program: &Pubkey,
    psm_pool: &Pubkey,
    psm_token: &Pubkey,
    psm_token_vault: &Pubkey,
    amount: u64,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);
    let permission_pda = derive_permission_pda(controller, super_authority);
    let reserve = derive_reserve_pda(controller, mint);
    let reserve_vault =
        get_associated_token_address_with_program_id(&controller_authority, mint, token_program);

    let remaining_accounts = vec![
        AccountMeta {
            pubkey: *psm_pool,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *psm_token,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *psm_token_vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: reserve_vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *token_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: spl_associated_token_account_client::program::ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: psm_client::PSM_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
    ];

    PushBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*super_authority)
        .permission(permission_pda)
        .integration(*integration)
        .reserve_a(reserve)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .push_args(PushArgs::PsmSwap { amount })
        .add_remaining_accounts(&remaining_accounts)
        .instruction()
}
