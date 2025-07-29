use litesvm::LiteSVM;
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    account::{Account as SolanaAccount, ReadableAccount},
    program_option::COption,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use spl_associated_token_account_client::{
    address::get_associated_token_address_with_program_id,
    instruction::create_associated_token_account_idempotent,
};
use spl_token_2022::{
    extension::StateWithExtensions,
    instruction::{initialize_mint2, mint_to},
    state::{Account, Mint},
};
use std::error::Error;

/// Unpacks a token account from either token program.
pub fn unpack_token_account(account: &SolanaAccount) -> Result<Account, String> {
    if account.owner() == &spl_token_2022::ID {
        StateWithExtensions::unpack(&account.data)
            .map(|a| a.base)
            .map_err(|e| format!("Failed to unpack token2022 account: {:?}", e))
    } else {
        Account::unpack(&account.data).map_err(|e| format!("Failed to unpack token account: {:?}", e))
    }
}

pub fn initialize_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
    decimals: u8,
    mint_kp: Option<Keypair>,
    token_program: &Pubkey,
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
        token_program,
    );

    let init_mint_ins = initialize_mint2(
        token_program,
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
    let token_program = svm.get_account(mint).unwrap().owner;
    let ata_pk = get_associated_token_address_with_program_id(owner, mint, &token_program);
    let create_ixn =
        create_associated_token_account_idempotent(&payer.pubkey(), owner, mint, &token_program);

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_ixn],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    ));
    assert!(tx_result.is_ok(), "Transaction failed to execute");

    let token_acc = svm.get_account(&ata_pk).unwrap();
    let token_account = unpack_token_account(&token_acc)?;

    assert_eq!(token_account.mint, *mint, "Incorrect ATA mint");
    assert_eq!(token_account.owner, *owner, "Incorrect ATA owner");

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
    let token_program = svm.get_account(mint).unwrap().owner;
    let ata_pk = get_associated_token_address_with_program_id(recipient, mint, &token_program);

    let balance_before = get_token_balance_or_zero(svm, &ata_pk);

    let create_ixn = create_associated_token_account_idempotent(
        &payer.pubkey(),
        recipient,
        mint,
        &token_program,
    );
    let mint_ixn = mint_to(
        &token_program,
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
        let token_account = unpack_token_account(&account);

        token_account.map_or(0, |acct| acct.amount)
    })
}

pub fn get_mint_supply_or_zero(svm: &mut LiteSVM, mint: &Pubkey) -> u64 {
    svm.get_account(mint).map_or(0, |account| {
        let mint = Mint::unpack(&account.data)
            .map_err(|e| format!("Failed to unpack token mint: {:?}", e));
        mint.map_or(0, |m| m.supply)
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
    let token_program = svm.get_account(mint).unwrap().owner;
    let source_ata_pk =
        get_associated_token_address_with_program_id(&authority.pubkey(), mint, &token_program);

    let destination_ata_pk =
        get_associated_token_address_with_program_id(recipient, mint, &token_program);

    let source_balance_before = get_token_balance_or_zero(svm, &source_ata_pk);
    let destination_balance_before = get_token_balance_or_zero(svm, &destination_ata_pk);

    let create_ixn = create_associated_token_account_idempotent(
        &payer.pubkey(),
        recipient,
        mint,
        &token_program,
    );
    let transfer_ixn = spl_token_2022::instruction::transfer(
        &token_program,
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
    let token_program = svm.get_account(mint).unwrap().owner;
    let ata_pk = get_associated_token_address_with_program_id(&owner, mint, &token_program);

    edit_token_amount(svm, &ata_pk, amount)?;

    let balance_after = get_token_balance_or_zero(svm, &ata_pk);

    assert_eq!(balance_after, amount, "balance_after is incorrect");

    Ok(())
}

pub fn edit_token_amount(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let mut account_info = svm.get_account(&pubkey).unwrap();

    let mut account = unpack_token_account(&account_info).unwrap();
    account.amount = amount;
    Account::pack(account, &mut account_info.data)?;
    svm.set_account(*pubkey, account_info)?;
    Ok(())
}
