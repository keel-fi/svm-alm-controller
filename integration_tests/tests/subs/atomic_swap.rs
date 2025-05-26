use std::error::Error;

use litesvm::LiteSVM;
use pinocchio_token::state::TokenAccount;
use solana_sdk::{
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use spl_token::state::Account;
use svm_alm_controller_client::generated::instructions::{
    AtomicSwapBorrowBuilder, CloseAtomicSwapBuilder,
};

use crate::subs::derive_reserve_pda;

pub fn fetch_token_account(svm: &mut LiteSVM, token_account: &Pubkey) -> Account {
    let info = svm.get_account(token_account).unwrap();
    Account::unpack(&info.data[..Account::LEN]).unwrap()
}

pub fn cancel_atomic_swap(
    svm: &mut LiteSVM,
    authority: &Keypair,
    controller: Pubkey,
    permission: Pubkey,
    integration: Pubkey,
) -> Result<(), Box<dyn Error>> {
    let ixn = CloseAtomicSwapBuilder::new()
        .payer(authority.pubkey())
        .controller(controller)
        .authority(authority.pubkey())
        .permission(permission)
        .integration(integration)
        .system_program(system_program::ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    let tx_result = svm.send_transaction(txn);
    assert!(tx_result.is_ok(), "Transaction failed: {:?}", tx_result);

    Ok(())
}

pub fn atomic_swap_borrow(
    svm: &mut LiteSVM,
    authority: &Keypair,
    controller: Pubkey,
    permission: Pubkey,
    integration: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    recipient_token_account: Pubkey,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let reserve_a = derive_reserve_pda(&controller, &mint_a);
    let reserve_b = derive_reserve_pda(&controller, &mint_b);
    let vault_a = get_associated_token_address_with_program_id(
        &controller,
        &mint_a,
        &pinocchio_token::ID.into(),
    );
    let vault_b = get_associated_token_address_with_program_id(
        &controller,
        &mint_b,
        &pinocchio_token::ID.into(),
    );

    let ixn = AtomicSwapBorrowBuilder::new()
        .controller(controller)
        .authority(authority.pubkey())
        .permission(permission)
        .integration(integration)
        .reserve_a(reserve_a)
        .vault_a(vault_a)
        .reserve_b(reserve_b)
        .vault_b(vault_b)
        .recipient_token_account(recipient_token_account)
        .token_program(pinocchio_token::ID.into())
        .amount(amount)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    let tx_result = svm.send_transaction(txn);
    assert!(tx_result.is_ok(), "Transaction failed: {:?}", tx_result);

    Ok(())
}
