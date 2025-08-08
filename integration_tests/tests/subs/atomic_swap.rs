use litesvm::{types::TransactionResult, LiteSVM};
use solana_sdk::{
    instruction::Instruction,
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use spl_token_2022::state::Account;
use svm_alm_controller_client::generated::instructions::{
    AtomicSwapBorrowBuilder, AtomicSwapRepayBuilder, RefreshOracleBuilder,
};

use crate::subs::{derive_controller_authority_pda, derive_reserve_pda};

pub fn fetch_token_account(svm: &mut LiteSVM, token_account: &Pubkey) -> Account {
    let info = svm.get_account(token_account).unwrap();
    Account::unpack(&info.data[..Account::LEN]).unwrap()
}

pub fn atomic_swap_borrow_repay_ixs(
    authority: &Keypair,
    controller: Pubkey,
    permission: Pubkey,
    integration: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    oracle: Pubkey,
    price_feed: Pubkey,
    payer_account_a: Pubkey,
    payer_account_b: Pubkey,
    token_program_a: Pubkey,
    token_program_b: Pubkey,
    repay_excess_token_a: bool,
    borrow_amount: u64,
    repay_amount: u64,
    mint_authority: &Keypair,
) -> [Instruction; 4] {
    let reserve_a = derive_reserve_pda(&controller, &mint_a);
    let reserve_b = derive_reserve_pda(&controller, &mint_b);
    let controller_authority = derive_controller_authority_pda(&controller);
    let vault_a = get_associated_token_address_with_program_id(
        &controller_authority,
        &mint_a,
        &token_program_a,
    );
    let vault_b = get_associated_token_address_with_program_id(
        &controller_authority,
        &mint_b,
        &token_program_b,
    );

    let refresh_ix = RefreshOracleBuilder::new()
        .oracle(oracle)
        .price_feed(price_feed)
        .instruction();

    let borrow_ix = AtomicSwapBorrowBuilder::new()
        .controller(controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(permission)
        .integration(integration)
        .reserve_a(reserve_a)
        .vault_a(vault_a)
        .mint_a(mint_a)
        .reserve_b(reserve_b)
        .vault_b(vault_b)
        .recipient_token_account_a(payer_account_a)
        .recipient_token_account_b(payer_account_b)
        .token_program_a(token_program_a)
        .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
        .repay_excess_token_a(repay_excess_token_a)
        .amount(borrow_amount)
        .instruction();

    let mint_ix = spl_token_2022::instruction::mint_to(
        &token_program_b,
        &mint_b,
        &payer_account_b,
        &mint_authority.pubkey(),
        &[],
        repay_amount,
    ).unwrap();

    let repay_ix = AtomicSwapRepayBuilder::new()
        .payer(authority.pubkey())
        .controller(controller)
        .authority(authority.pubkey())
        .permission(permission)
        .integration(integration)
        .reserve_a(reserve_a)
        .vault_a(vault_a)
        .mint_a(mint_a)
        .reserve_b(reserve_b)
        .vault_b(vault_b)
        .mint_b(mint_b)
        .oracle(oracle)
        .payer_account_a(payer_account_a)
        .payer_account_b(payer_account_b)
        .token_program_a(token_program_a)
        .token_program_b(token_program_b)
        .instruction();
    [borrow_ix, refresh_ix, mint_ix, repay_ix]
}

pub fn atomic_swap_borrow_repay(
    svm: &mut LiteSVM,
    authority: &Keypair,
    controller: Pubkey,
    permission: Pubkey,
    integration: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    oracle: Pubkey,
    price_feed: Pubkey,
    payer_account_a: Pubkey,
    payer_account_b: Pubkey,
    token_program_a: Pubkey,
    token_program_b: Pubkey,
    repay_excess_token_a: bool,
    borrow_amount: u64,
    repay_amount: u64,
    mint_authority: &Keypair,
) -> TransactionResult {
    let instructions = atomic_swap_borrow_repay_ixs(
        authority,
        controller,
        permission,
        integration,
        mint_a,
        mint_b,
        oracle,
        price_feed,
        payer_account_a,
        payer_account_b,
        token_program_a,
        token_program_b,
        repay_excess_token_a,
        borrow_amount,
        repay_amount,
        mint_authority,
    );
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&authority.pubkey()),
        &[&authority, &mint_authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(txn)
}
