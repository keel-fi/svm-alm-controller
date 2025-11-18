use litesvm::LiteSVM;
use psm_client::{create_add_token_instruction, create_initialize_pool_instruction, derive_psm_pool_pda, types::TokenStatus};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, transaction::Transaction};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use spl_token::ID as TOKEN_PROGRAM_ID;
use spl_token_2022::ID as TOKEN_2022_ID;
use psm_client::derive_token_pda;
use spl_associated_token_account_client::program::ID as ASSOCIATED_TOKEN_PROGRAM_ID;


/// Initialize a PSM pool and add a single active token with default limits
pub fn setup_pool_with_token(
    svm: &mut LiteSVM,
    payer_keypair: &Keypair,
    mint_pubkey: &Pubkey,
    token2022: bool,
    invalid_extension: bool,
    liquidity_owner: &Pubkey
) -> (
    Pubkey,  // pool_pda
    Pubkey,  // token_pda
    Pubkey, // vault
) {
    let freeze_authority_keypair = Keypair::new();
    let pricing_authority1_keypair = Keypair::new();
    let pricing_authority2_keypair = Keypair::new();
    let pricing_authority3_keypair = Keypair::new();

    svm.airdrop(&payer_keypair.pubkey(), 10_000_000_000)
        .unwrap();

    let salt = 0u16;
    let (pool_pda, _bump) = derive_psm_pool_pda(salt);

    let instruction = create_initialize_pool_instruction(
        payer_keypair.pubkey(),
        payer_keypair.pubkey(),
        freeze_authority_keypair.pubkey(),
        pricing_authority1_keypair.pubkey(),
        pricing_authority2_keypair.pubkey(),
        pricing_authority3_keypair.pubkey(),
        *liquidity_owner,
        pool_pda,
        salt,
    );

    let blockhash = svm.latest_blockhash();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer_keypair.insecure_clone().pubkey()),
        &[payer_keypair.insecure_clone()],
        blockhash,
    );
    svm.send_transaction(transaction).unwrap();

    // Create a mint for the token
    let (token_program, _transfer_hook_enabled) = if token2022 {
        let transfer_hook_enabled = if invalid_extension {
            Some(true)
        } else { 
            Some(false) 
        };
        (TOKEN_2022_ID, transfer_hook_enabled)
    } else {
        (TOKEN_PROGRAM_ID, None)
    };

    let (token_pda, _bump) = derive_token_pda(mint_pubkey, &pool_pda);
    let vault_address = get_associated_token_address_with_program_id(
        &pool_pda, 
        mint_pubkey,
        &token_program
    );

    let token_instruction = create_add_token_instruction(
        payer_keypair.pubkey(),
        payer_keypair.pubkey(),
        pool_pda,
        token_pda,
        *mint_pubkey,
        vault_address,
        TokenStatus::Active as u8,
        1000_000_000,
        1000_000_000,
        system_program::id(),
        token_program,
        ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let blockhash = svm.latest_blockhash();
    let transaction = Transaction::new_signed_with_payer(
        &[token_instruction],
        Some(&payer_keypair.insecure_clone().pubkey()),
        &[payer_keypair.insecure_clone()],
        blockhash,
    );
    svm.send_transaction(transaction).unwrap();

    (
        pool_pda,
        token_pda,
        vault_address,
    )
}