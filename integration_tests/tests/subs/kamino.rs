use std::error::Error;

use litesvm::LiteSVM;
use solana_program::hash;
use solana_sdk::{
    clock::Clock, instruction::{AccountMeta, Instruction}, pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction
};

use crate::helpers::constants::KAMINO_LEND_PROGRAM_ID;


pub fn derive_vanilla_obligation_address(
    obligation_id: u8,
    authority: &Pubkey,
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (obligation_pda, _) = Pubkey::find_program_address(
        &[
            // tag 0 for vanilla obligation
            &0_u8.to_le_bytes(),
            // id 0 as default
            &obligation_id.to_le_bytes(),
            // user
            authority.as_ref(),
            // kamino market
            market.as_ref(),
            // seed 1, for lending obligation is the token
            Pubkey::default().as_ref(),
            // seed 2, for lending obligation is the token
            Pubkey::default().as_ref(),
        ],
        kamino_program
    );

    obligation_pda
}

fn derive_anchor_discriminator(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);

    let mut sighash = [0_u8; 8];
    sighash.copy_from_slice(
        &hash::hash(preimage.as_bytes()).to_bytes()[..8]
    );

    sighash
}

pub fn refresh_reserve(
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
pub fn refresh_obligation(
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