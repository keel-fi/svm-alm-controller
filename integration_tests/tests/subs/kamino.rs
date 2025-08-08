use std::error::Error;

use litesvm::LiteSVM;
use solana_program::hash;
use solana_sdk::{
    instruction::{AccountMeta, Instruction}, pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction
};

use crate::helpers::constants::KAMINO_LEND_PROGRAM_ID;


pub fn derive_market_authority_address(
    market: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"lma",
            market.as_ref(),
        ], 
        kamino_program
    );

    address
}

pub fn derive_user_metadata_address(
    user: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"user_meta",
            &user.as_ref()
        ], 
        &kamino_program
    );

    address
}

pub fn derive_lookup_table_address(
    authority_address: &Pubkey,
    recent_block_slot: u64,
    lookup_table_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            authority_address.as_ref(),
            &recent_block_slot.to_le_bytes()
        ], 
        lookup_table_program
    );

    address
}

pub fn derive_obligation_farm_address(
    reserve_farm: &Pubkey, 
    obligation: &Pubkey,
    kamino_farms_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"user",
            reserve_farm.as_ref(),
            &obligation.as_ref()
        ], 
        &kamino_farms_program
    );

    address
}

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

pub fn derive_reserve_collateral_mint(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_coll_mint",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        kamino_program
    );

    address
}

pub fn derive_reserve_collateral_supply(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_coll_supply",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        kamino_program
    );

    address
}

pub fn derive_reserve_liquidity_supply(
    market: &Pubkey,
    reserve_liquidity_mint: &Pubkey,
    kamino_program: &Pubkey
) -> Pubkey {
    let (address, _) = Pubkey::find_program_address(
        &[
            b"reserve_liq_supply",
            market.as_ref(), 
            reserve_liquidity_mint.as_ref()
        ], 
        kamino_program
    );

    address
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
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    Ok(())
}

pub fn refresh_obligation(
    svm: &mut LiteSVM,
    payer: &Keypair,
    market: &Pubkey,
    obligation: &Pubkey,
) -> Result<(), Box<dyn Error>> {
    let data = derive_anchor_discriminator(
        "global", 
        "refresh_obligation"
    );

    let instruction = Instruction {
        program_id: KAMINO_LEND_PROGRAM_ID,
        accounts: vec![
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
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    Ok(())
}