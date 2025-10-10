use std::error::Error;

use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction}, 
    pubkey::Pubkey, signature::Keypair, 
    signer::Signer, transaction::Transaction
};
use svm_alm_controller_client::integrations::kamino::{derive_anchor_discriminator, KaminoReserve, Obligation};

use crate::helpers::constants::KAMINO_LEND_PROGRAM_ID;

pub fn get_liquidity_and_lp_amount(
    svm: &mut LiteSVM,
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

pub fn refresh_kamino_reserve(
    svm: &mut LiteSVM,
    payer: &Keypair,
    reserve: &Pubkey,
    market: &Pubkey,
    scope_prices: &Pubkey,
) -> Result<(), Box<dyn Error>> {

    let data = derive_anchor_discriminator(
        "global", 
        "refresh_reserve"
    );

    let instruction = Instruction {
        program_id: KAMINO_LEND_PROGRAM_ID,
        accounts: vec![
            AccountMeta {
                pubkey: *reserve,
                is_signer: false,
                is_writable: true
            },
            AccountMeta {
                pubkey: *market,
                is_signer: false,
                is_writable: false,
            },
            // pyth oracle
            AccountMeta {
                pubkey: KAMINO_LEND_PROGRAM_ID,
                is_signer: false,
                is_writable: false
            },
            // switchboard_price_oracle
            AccountMeta {
                pubkey: KAMINO_LEND_PROGRAM_ID,
                is_signer: false,
                is_writable: false
            },
            // switchboard_twap_oracle
            AccountMeta {
                pubkey: KAMINO_LEND_PROGRAM_ID,
                is_signer: false,
                is_writable: false
            },
            // scope_prices
            AccountMeta {
                pubkey: *scope_prices,
                is_signer: false,
                is_writable: false
            }
        ],
        data: data.to_vec()
    };
    
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
    let data = derive_anchor_discriminator(
        "global", 
        "refresh_obligation"
    );

    let mut accounts = vec![
        AccountMeta {
            pubkey: *market,
            is_signer: false,
            is_writable: false
        },
        AccountMeta {
            pubkey: *obligation,
            is_signer: false,
            is_writable: true
        }
    ];

    match reserve {
        Some(reserve) => {
            accounts.push(
                AccountMeta { 
                    pubkey: *reserve, 
                    is_signer: false, 
                    is_writable: true
                }
            )
        },
        None => ()
    }



    let instruction = Instruction {
        program_id: KAMINO_LEND_PROGRAM_ID,
        accounts: accounts,
        data: data.to_vec()
    };

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