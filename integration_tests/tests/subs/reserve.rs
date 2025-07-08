use borsh::{de, BorshDeserialize};
use litesvm::LiteSVM;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, transaction::Transaction,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use std::error::Error;
use svm_alm_controller_client::generated::{
    accounts::Reserve,
    instructions::{InitializeReserveBuilder, ManageReserveBuilder},
    programs::SVM_ALM_CONTROLLER_ID,
    types::ReserveStatus,
};

use crate::subs::{derive_controller_authority_pda, derive_permission_pda};

pub fn derive_reserve_pda(controller_pda: &Pubkey, mint: &Pubkey) -> Pubkey {
    let (reserve_pda, _reserve_bump) = Pubkey::find_program_address(
        &[b"reserve", &controller_pda.to_bytes(), &mint.to_bytes()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    reserve_pda
}

pub fn fetch_reserve_account(
    svm: &mut LiteSVM,
    reserve_pda: &Pubkey,
) -> Result<Option<Reserve>, Box<dyn Error>> {
    let info = svm.get_account(reserve_pda);
    match info {
        Some(info) => {
            if info.data.is_empty() {
                Ok(None)
            } else {
                Reserve::try_from_slice(&info.data[1..])
                    .map(Some)
                    .map_err(Into::into)
            }
        }
        None => Ok(None),
    }
}

pub struct ReserveKeys {
    pub pubkey: Pubkey,
    pub vault: Pubkey,
}
pub fn initialize_reserve(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    mint: &Pubkey,
    payer: &Keypair,
    authority: &Keypair,
    status: ReserveStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
) -> Result<ReserveKeys, Box<dyn Error>> {
    let calling_permission_pda: Pubkey = derive_permission_pda(controller, &authority.pubkey());

    let reserve_pda = derive_reserve_pda(controller, mint);

    let controller_authority = derive_controller_authority_pda(controller);

    let vault = get_associated_token_address_with_program_id(
        &controller_authority,
        mint,
        &pinocchio_token::ID.into(),
    );

    let ixn = InitializeReserveBuilder::new()
        .status(status)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .payer(payer.pubkey())
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .reserve(reserve_pda)
        .mint(*mint)
        .vault(vault)
        .token_program(pinocchio_token::ID.into())
        .associated_token_program(pinocchio_associated_token_account::ID.into())
        .system_program(system_program::ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&authority, &payer],
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn);
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let reserve =
        fetch_reserve_account(svm, &reserve_pda).expect("Failed to fetch reserve account");
    assert!(
        reserve.is_some(),
        "Reserve must exist after the transaction"
    );

    let reserve = reserve.unwrap();
    assert_eq!(
        reserve.status, status,
        "Status does not match expected value"
    );
    assert_eq!(
        reserve.rate_limit_slope, rate_limit_slope,
        "Rate limit slope does not match expected value"
    );
    assert_eq!(
        reserve.rate_limit_max_outflow, rate_limit_max_outflow,
        "Rate limit max outflow does not match expected value"
    );
    assert_eq!(
        reserve.controller, *controller,
        "Controller does not match expected value"
    );
    assert_eq!(reserve.mint, *mint, "Mint does not match expected value");
    assert_eq!(reserve.vault, vault, "Vault does not match expected value");

    Ok(ReserveKeys {
        pubkey: reserve_pda,
        vault,
    })
}

pub fn manage_reserve(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    mint: &Pubkey,
    authority: &Keypair,
    status: ReserveStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
) -> Result<(), Box<dyn Error>> {
    let calling_permission_pda = derive_permission_pda(controller, &authority.pubkey());
    let controller_authority = derive_controller_authority_pda(controller);

    let reserve_pda = derive_reserve_pda(controller, mint);

    let ixn = ManageReserveBuilder::new()
        .status(status)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .reserve(reserve_pda)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let tx_result = svm.send_transaction(txn);
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let reserve =
        fetch_reserve_account(svm, &reserve_pda).expect("Failed to fetch reserve account");
    assert!(
        reserve.is_some(),
        "Reserve must exist after the transaction"
    );

    let reserve = reserve.unwrap();
    assert_eq!(
        reserve.status, status,
        "Status does not match expected value"
    );
    assert_eq!(
        reserve.rate_limit_slope, rate_limit_slope,
        "Rate limit slope does not match expected value"
    );
    assert_eq!(
        reserve.rate_limit_max_outflow, rate_limit_max_outflow,
        "Rate limit max outflow does not match expected value"
    );
    assert_eq!(
        reserve.controller, *controller,
        "Controller does not match expected value"
    );

    Ok(())
}
