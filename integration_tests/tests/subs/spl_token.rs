use litesvm::LiteSVM;
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    account::ReadableAccount, program_option::COption, pubkey::Pubkey, signature::Keypair,
    signer::Signer, transaction::Transaction,
};
use spl_associated_token_account_client::{
    address::get_associated_token_address_with_program_id,
    instruction::create_associated_token_account_idempotent,
};
use spl_token::{
    instruction::{initialize_mint2, mint_to},
    state::{Account, Mint},
};
use std::error::Error;

pub fn initialize_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
    decimals: u8,
    mint_kp: Option<Keypair>,
) -> Result<Pubkey, Box<dyn Error>> {
    let mint_kp = if mint_kp.is_some() {
        mint_kp.unwrap()
    } else {
        Keypair::new()
    };
    let mint_pk = mint_kp.pubkey();
    let mint_len = Mint::LEN;

    let create_acc_ins = solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &mint_pk,
        svm.minimum_balance_for_rent_exemption(mint_len),
        mint_len as u64,
        &spl_token::id(),
    );

    let init_mint_ins = initialize_mint2(
        &spl_token::id(),
        &mint_pk,
        mint_authority,
        freeze_authority,
        decimals,
    )
    .unwrap();

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_acc_ins, init_mint_ins],
        Some(&payer.pubkey()),
        &[&payer, &mint_kp],
        svm.latest_blockhash(),
    ));
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let mint_acc = svm.get_account(&mint_kp.pubkey());
    let mint_data = mint_acc.unwrap().data;
    let mint = Mint::unpack(&mint_data).map_err(|e| format!("Failed to unpack mint: {:?}", e))?;

    assert_eq!(mint.decimals, decimals, "Incorrect number of decimals");
    assert_eq!(
        mint.mint_authority,
        COption::Some(*mint_authority),
        "Incorrect mint_authority"
    );
    assert_eq!(
        mint.freeze_authority,
        freeze_authority
            .map(|fa| COption::Some(*fa))
            .unwrap_or(COption::None),
        "Incorrect freeze_authority"
    );

    Ok(mint_pk)
}

pub fn initialize_ata(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Result<Pubkey, Box<dyn Error>> {
    let ata_pk = get_associated_token_address_with_program_id(owner, mint, &spl_token::id());
    let create_ixn =
        create_associated_token_account_idempotent(&payer.pubkey(), owner, mint, &spl_token::id());

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_ixn],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    ));
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let ata_acc = svm.get_account(&ata_pk);
    let ata_data = ata_acc.unwrap().data;
    let ata = Account::unpack(&ata_data).map_err(|e| format!("Failed to unpack ata: {:?}", e))?;

    assert_eq!(ata.mint, *mint, "Incorrect ATA mint");
    assert_eq!(ata.owner, *owner, "Incorrect ATA owner");

    Ok(ata_pk)
}

pub fn mint_tokens(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_authority: &Keypair,
    mint: &Pubkey,
    recipient: &Pubkey,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let ata_pk = get_associated_token_address_with_program_id(recipient, mint, &spl_token::id());

    let balance_before = get_token_balance_or_zero(svm, &ata_pk);

    let create_ixn = create_associated_token_account_idempotent(
        &payer.pubkey(),
        recipient,
        mint,
        &spl_token::id(),
    );
    let mint_ixn = mint_to(
        &spl_token::id(),
        mint,
        &ata_pk,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey()],
        amount,
    )?;

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_ixn, mint_ixn],
        Some(&payer.pubkey()),
        &[&payer, &mint_authority],
        svm.latest_blockhash(),
    ));
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let balance_after = get_token_balance_or_zero(svm, &ata_pk);
    let delta = balance_after.checked_sub(balance_before).unwrap();

    assert_eq!(delta, amount, "Amount minted is incorrect");

    Ok(())
}

pub fn get_token_balance_or_zero(svm: &mut LiteSVM, token_account: &Pubkey) -> u64 {
    svm.get_account(token_account).map_or(0, |account| {
        let ata =
            Account::unpack(&account.data).map_err(|e| format!("Failed to unpack ata: {:?}", e));
        ata.map_or(0, |ata| ata.amount)
    })
}

pub fn get_mint_supply_or_zero(svm: &mut LiteSVM, mint: &Pubkey) -> u64 {
    svm.get_account(mint).map_or(0, |account| {
        let ata = Mint::unpack(&account.data).map_err(|e| format!("Failed to unpack ata: {:?}", e));
        ata.map_or(0, |ata| ata.supply)
    })
}

pub fn transfer_tokens(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    mint: &Pubkey,
    recipient: &Pubkey,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let source_ata_pk =
        get_associated_token_address_with_program_id(&authority.pubkey(), mint, &spl_token::id());

    let destination_ata_pk =
        get_associated_token_address_with_program_id(recipient, mint, &spl_token::id());

    let source_balance_before = get_token_balance_or_zero(svm, &source_ata_pk);
    let destination_balance_before = get_token_balance_or_zero(svm, &destination_ata_pk);

    let create_ixn = create_associated_token_account_idempotent(
        &payer.pubkey(),
        recipient,
        mint,
        &spl_token::id(),
    );
    let transfer_ixn = spl_token::instruction::transfer(
        &spl_token::id(),
        &source_ata_pk,
        &destination_ata_pk,
        &authority.pubkey(),
        &[&authority.pubkey()],
        amount,
    )?;

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_ixn, transfer_ixn],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        svm.latest_blockhash(),
    ));
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let source_balance_after = get_token_balance_or_zero(svm, &source_ata_pk);
    let destination_balance_after = get_token_balance_or_zero(svm, &destination_ata_pk);

    let source_delta = source_balance_before
        .checked_sub(source_balance_after)
        .unwrap();
    let destination_delta = destination_balance_after
        .checked_sub(destination_balance_before)
        .unwrap();

    assert_eq!(
        source_delta, amount,
        "Amount deducted from source is incorrect"
    );
    assert_eq!(
        destination_delta, amount,
        "Amount added to destination is incorrect"
    );

    Ok(())
}

pub fn edit_ata_amount(
    svm: &mut LiteSVM,
    owner: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let ata_pk = get_associated_token_address_with_program_id(&owner, mint, &spl_token::id());

    let mut ata_ai = svm.get_account(&ata_pk).unwrap();

    let mut ata = Account::unpack(&ata_ai.data)
        .map_err(|e| format!("Failed to unpack ata: {:?}", e))
        .unwrap();
    ata.amount = amount;
    Account::pack(ata, &mut ata_ai.data)?;
    svm.set_account(ata_pk, ata_ai)?;

    let balance_after = get_token_balance_or_zero(svm, &ata_pk);

    assert_eq!(balance_after, amount, "balance_after is incorrect");

    Ok(())
}
