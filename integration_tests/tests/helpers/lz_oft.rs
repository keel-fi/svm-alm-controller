use std::error::Error;

use oft_client::{
    instructions::SendInstructionArgs,
    oft302::{Oft302, Oft302SendAccounts, Oft302SendPrograms},
};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, sysvar};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::{
    create_lz_bridge_push_instruction, generated::instructions::ResetLzPushInFlightBuilder,
};

use crate::helpers::constants::{DEVNET_RPC, LZ_ENDPOINT_PROGRAM_ID, LZ_USDS_ESCROW};

// NOTE: This uses hardcoded values and is not suited for use
// outside of testing.
pub async fn create_send_ix(
    payer: &Pubkey,
    oft_program_id: &Pubkey,
    token_program_id: &Pubkey,
    mint: &Pubkey,
    destination_address: &Pubkey,
    destination_eid: u32,
    amount: u64,
) -> Result<Instruction, Box<dyn Error>> {
    let payer_ata = get_associated_token_address_with_program_id(payer, mint, token_program_id);
    let oft302: Oft302 = Oft302::new(*oft_program_id, DEVNET_RPC.to_owned());

    let send_accs = Oft302SendAccounts {
        payer: *payer,
        token_mint: *mint,
        token_escrow: LZ_USDS_ESCROW,
        token_source: payer_ata,
        peer_address: None,
    };
    let send_params = SendInstructionArgs {
        dst_eid: destination_eid,
        to: destination_address.to_bytes(),
        amount_ld: amount,
        min_amount_ld: amount,
        options: vec![],
        compose_msg: None,
        // value read from program in LiteSVM env
        native_fee: 1025646,
        lz_token_fee: 0,
    };
    let send_programs = Oft302SendPrograms {
        endpoint: Some(LZ_ENDPOINT_PROGRAM_ID),
        token: Some(pinocchio_token::ID.into()),
    };

    let send_ix = oft302
        .send(send_accs, send_params, send_programs, vec![])
        .await?;

    Ok(send_ix)
}

pub async fn create_lz_push_and_send_ixs(
    controller: &Pubkey,
    authority: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
    oft_program_id: &Pubkey,
    token_program: &Pubkey,
    destination_address: &Pubkey,
    destination_eid: u32,
    mint: &Pubkey,
    amount: u64,
) -> Result<[Instruction; 3], Box<dyn Error>> {
    let push_ix = create_lz_bridge_push_instruction(
        controller,
        authority,
        integration,
        reserve,
        token_program,
        mint,
        amount,
    );
    let integration_pubkey = push_ix.accounts[4].pubkey;

    let send_ix = create_send_ix(
        authority,
        oft_program_id,
        token_program,
        mint,
        destination_address,
        destination_eid,
        amount,
    )
    .await?;

    let reset_ix = ResetLzPushInFlightBuilder::new()
        .controller(*controller)
        .sysvar_instruction(sysvar::instructions::ID)
        .integration(integration_pubkey)
        .instruction();

    Ok([push_ix, send_ix, reset_ix])
}
