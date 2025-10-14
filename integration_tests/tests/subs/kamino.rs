use std::error::Error;

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account, 
    pubkey::Pubkey, 
    signature::Keypair, 
    signer::Signer, 
    transaction::Transaction
};
use svm_alm_controller_client::{
    create_refresh_kamino_obligation_instruction, 
    create_refresh_kamino_reserve_instruction, 
    integrations::kamino::{KaminoReserve, Obligation, FARM_DEBT_OFFSET, LIQUIDITY_AVAILABLE_AMOUNT_OFFSET}};


pub fn get_liquidity_and_lp_amount(
    svm: &LiteSVM,
    kamino_reserve_pk: &Pubkey,
    obligation_pk: &Pubkey,
) -> Result<(u64, u64), Box<dyn std::error::Error>> {
    let obligation_acc = svm.get_account(obligation_pk)
        .expect("could not get obligation");

    let obligation_state = Obligation::try_deserialize(&obligation_acc.data)?;

    // if the obligation is closed 
    // (there has been a full withdrawal and it only had one ObligationCollateral slot used),
    // then the lp_amount is 0
    let is_obligation_closed = obligation_acc.lamports == 0;

    let lp_amount = if is_obligation_closed { 0 } else {
        // if it's not closed, then we read the state,
        // but its possible that the ObligationCollateral hasn't been created yet (first deposit)
        // in that case lp_amount is also 0

        // handles the case where no ObligationCollateral is found
        obligation_state.get_obligation_collateral_for_reserve(kamino_reserve_pk)
            .map_or(0, |collateral| collateral.deposited_amount)
    };

    // avoids deserializing kamino_reserve if lp_amount is 0
    let liquidity_value = if lp_amount == 0 { 0 } else {
        let kamino_reserve_acc = svm.get_account(kamino_reserve_pk)
        .expect("could not get kamino reserve");
        let kamino_reserve_state = KaminoReserve::try_deserialize(&kamino_reserve_acc.data)?;
        kamino_reserve_state.collateral_to_liquidity(lp_amount)
    };

    Ok((liquidity_value, lp_amount))
}

pub fn fetch_kamino_reserve(
    svm: &LiteSVM,
    kamino_reserve_pk: &Pubkey,
) -> Result<KaminoReserve, Box<dyn std::error::Error>> {
    let acc = svm.get_account(kamino_reserve_pk)
        .expect("failed to get kamino account");

    let kamino_reserve = KaminoReserve::try_deserialize(&acc.data)?;

    Ok(kamino_reserve)
}

pub fn set_kamino_reserve_liquidity_available_amount(
    svm: &mut LiteSVM,
    kamino_reserve_pk: &Pubkey,
    amount: u64
) -> Result<(), Box<dyn std::error::Error>> {
    let acc = svm.get_account(kamino_reserve_pk)
        .expect("failed to get kamino reserve ");

    svm.set_account(*kamino_reserve_pk, Account {
        data : vec![
            acc.data[..LIQUIDITY_AVAILABLE_AMOUNT_OFFSET].to_vec(),
            amount.to_le_bytes().to_vec(),
            acc.data[LIQUIDITY_AVAILABLE_AMOUNT_OFFSET + 8..].to_vec()
        ].concat(),
        ..acc
    }).expect("failed to set kamino reserve ");

    Ok(())
}

pub fn refresh_kamino_reserve(
    svm: &mut LiteSVM,
    payer: &Keypair,
    reserve: &Pubkey,
    market: &Pubkey,
    scope_prices: &Pubkey,
) -> Result<(), Box<dyn Error>> {
    let instruction = create_refresh_kamino_reserve_instruction(
        reserve, 
        market, 
        scope_prices
    );
    
    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[instruction], 
        Some(&payer.pubkey()), 
        &[&payer], 
        svm.latest_blockhash()
    ));

    if tx_result.is_err() {
        println!("{:#?}", tx_result.unwrap().logs);
    } else {
        match &tx_result {
            Ok(result) => {
                println!("tx signature: {}", result.signature.to_string())
            },
            _ => ()
        }
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    Ok(())
}

/// If obligation has reserves, they need to be added as remaining accounts!
/// for the sake of simplicity, this method only support obligations with 1 reserve.
pub fn refresh_kamino_obligation(
    svm: &mut LiteSVM,
    payer: &Keypair,
    market: &Pubkey,
    obligation: &Pubkey,
    reserve: Option<&Pubkey>
) -> Result<(), Box<dyn Error>> {
    let instruction = create_refresh_kamino_obligation_instruction(
        market, 
        obligation, 
        reserve
    );

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[instruction], 
        Some(&payer.pubkey()), 
        &[&payer], 
        svm.latest_blockhash()
    ));

    if tx_result.is_err() {
        println!("{:#?}", tx_result.unwrap().logs);
    } else {
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    Ok(())
}