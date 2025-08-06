use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};

use crate::helpers::{constants::TEST_TRANSFER_HOOK_PROGRAM_ID, spl::ASSOCIATED_TOKEN_PROGRAM_ID};

/// Initialize the ExtraAccountMetaList for a TransferHook for the given mint.
pub fn initialize_extra_metas(svm: &mut LiteSVM, mint: &Pubkey, token_program: &Pubkey) {
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000);

    let counter_address =
        Pubkey::find_program_address(&[b"counter"], &TEST_TRANSFER_HOOK_PROGRAM_ID).0;
    let extra_metas_address = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        &TEST_TRANSFER_HOOK_PROGRAM_ID,
    )
    .0;

    // Metas for `InitializeExtraAccountMetaList`
    let account_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(extra_metas_address, false),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(counter_address, false),
        AccountMeta::new_readonly(*token_program, false),
        AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];

    // Instruction for `InitializeExtraAccountMetaList`
    let ix = Instruction {
        program_id: TEST_TRANSFER_HOOK_PROGRAM_ID,
        accounts: account_metas,
        data: vec![],
    };

    svm.send_transaction(Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    ))
    .unwrap();
}
