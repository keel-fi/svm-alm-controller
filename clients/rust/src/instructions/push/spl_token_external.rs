use solana_instruction::{AccountMeta, Instruction};
use solana_program::system_program;
use solana_pubkey::Pubkey;
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::PushBuilder, types::PushArgs},
};

/// Instruction generation for SplTokenExternal "Push".
pub fn create_spl_token_external_push_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
    token_program: &Pubkey,
    mint: &Pubkey,
    recipient: &Pubkey,
    amount: u64,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);
    let permission_pda = derive_permission_pda(controller, authority);
    let vault =
        get_associated_token_address_with_program_id(&controller_authority, mint, token_program);
    let recipient_ata =
        get_associated_token_address_with_program_id(recipient, mint, token_program);

    let remaining_accounts = [
        AccountMeta {
            pubkey: *mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *recipient,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: recipient_ata,
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
            pubkey: system_program::ID,
            is_signer: false,
            is_writable: false,
        },
    ];
    PushBuilder::new()
        .push_args(PushArgs::SplTokenExternal { amount })
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(permission_pda)
        .integration(*integration)
        .reserve_a(*reserve)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .add_remaining_accounts(&remaining_accounts)
        .instruction()
}
