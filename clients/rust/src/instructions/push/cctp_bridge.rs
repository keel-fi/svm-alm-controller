use solana_instruction::{AccountMeta, Instruction};
use solana_program::system_program;
use solana_pubkey::Pubkey;
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::PushBuilder, types::PushArgs},
    integrations::cctp_bridge,
    SPL_TOKEN_PROGRAM_ID,
};

/// Instruction generation for CCTP "Push".
pub fn create_cctp_bridge_push_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
    message_sent_event_data: &Pubkey,
    mint: &Pubkey,
    destination_domain: u32,
    amount: u64,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);
    let permission_pda = derive_permission_pda(controller, authority);
    let vault = get_associated_token_address_with_program_id(
        &controller_authority,
        mint,
        &SPL_TOKEN_PROGRAM_ID,
    );

    let remaining_accounts = [
        AccountMeta {
            pubkey: *mint,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: cctp_bridge::derive_sender_authority_pda(
                &cctp_bridge::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            ),
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: cctp_bridge::derive_message_transmitter_pda(
                &cctp_bridge::CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID,
            ),
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: cctp_bridge::derive_token_messenger_pda(
                &cctp_bridge::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            ),
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: cctp_bridge::derive_remote_token_messenger_pda(
                &destination_domain.to_string(),
                &cctp_bridge::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            ),
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: cctp_bridge::derive_token_minter_pda(
                &cctp_bridge::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            ),
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: cctp_bridge::derive_local_token_pda(
                mint,
                &cctp_bridge::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            ),
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *message_sent_event_data,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: cctp_bridge::CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: cctp_bridge::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: cctp_bridge::derive_event_authority_pda(
                &cctp_bridge::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
            ),
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: SPL_TOKEN_PROGRAM_ID,
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
        .push_args(PushArgs::CctpBridge { amount })
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(permission_pda)
        .integration(*integration)
        .reserve_a(*reserve)
        .reserve_b(*reserve)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .add_remaining_accounts(&remaining_accounts)
        .instruction()
}
