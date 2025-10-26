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
// use spl_token_2022::extension::pausable::instruction::pause;
use spl_token_2022::extension::transfer_hook::instruction::initialize as initialize_transfer_hook;
use spl_token_2022::{
    extension::{
        pausable::instruction::initialize as initialize_pausable,
        transfer_fee::instruction::initialize_transfer_fee_config, ExtensionType,
        StateWithExtensions,
    },
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
        Account::unpack(&account.data)
            .map_err(|e| format!("Failed to unpack token account: {:?}", e))
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
    transfer_fee_bps: Option<u16>,
    transfer_hook_enabled: Option<bool>,
) -> Result<Pubkey, Box<dyn Error>> {
    let mint_kp = if mint_kp.is_some() {
        mint_kp.unwrap()
    } else {
        Keypair::new()
    };
    let mint_pubkey = mint_kp.pubkey();

    let mut instructions = Vec::new();

    // Track the extensions required for size calculation
    let mut extension_types = vec![];

    // Init the TransferFee token extension if fee is set AND
    // insert the TransferFee extension to extension types.
    if let Some(fee) = transfer_fee_bps {
        extension_types.push(ExtensionType::TransferFeeConfig);
        let init_transfer_fee_ix = initialize_transfer_fee_config(
            token_program,
            &mint_pubkey,
            Some(&payer.pubkey()),
            Some(&payer.pubkey()),
            fee,
            u64::MAX,
        )
        .unwrap();
        instructions.push(init_transfer_fee_ix);
    }

    if let Some(transfer_program) = transfer_hook_enabled {
        extension_types.push(ExtensionType::TransferHook);
        let init_transfer_hook = initialize_transfer_hook(
            token_program,
            &mint_pubkey,
            Some(payer.pubkey()),
            if transfer_program {
                Some(Pubkey::new_unique())
            } else {
                None
            },
        )
        .unwrap();
        instructions.push(init_transfer_hook);
    }

    let space = ExtensionType::try_calculate_account_len::<Mint>(&extension_types).unwrap();

    let create_acc_ins = solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &mint_pubkey,
        svm.minimum_balance_for_rent_exemption(space),
        space as u64,
        token_program,
    );
    instructions.insert(0, create_acc_ins);

    let init_mint_inx = initialize_mint2(
        token_program,
        &mint_pubkey,
        mint_authority,
        freeze_authority,
        decimals,
    )
    .unwrap();
    instructions.push(init_mint_inx);

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[&payer, &mint_kp],
        svm.latest_blockhash(),
    ));
    match tx_result.clone() {
        Ok(_res) => {}
        Err(e) => {
            panic!("Transaction errored\n{:?}", e.meta.logs);
        }
    }
    // assert!(tx_result.is_ok(), "Transaction failed to execute");

    let mint_acc = svm.get_account(&mint_kp.pubkey());
    let mint_data = mint_acc.unwrap().data;
    let mint = StateWithExtensions::<Mint>::unpack(&mint_data)
        .map_err(|e| format!("Failed to unpack mint: {:?}", e))?
        .base;

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

    Ok(mint_pubkey)
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

pub fn get_token_balance_or_zero(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    svm.get_account(token_account).map_or(0, |account| {
        let token_account = unpack_token_account(&account);

        token_account.map_or(0, |acct| acct.amount)
    })
}

/// Get the Mint account for a given mint pubkey
pub fn get_mint(svm: &LiteSVM, mint: &Pubkey) -> Mint {
    let mint_acc = svm.get_account(mint).unwrap();
    StateWithExtensions::<Mint>::unpack(&mint_acc.data)
        .expect("Failed to unpack mint")
        .base
}

pub fn get_mint_supply_or_zero(svm: &LiteSVM, mint: &Pubkey) -> u64 {
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
    let mint_acc = svm.get_account(mint).unwrap();
    let token_program = mint_acc.owner;
    let mint_state = StateWithExtensions::<Mint>::unpack(&mint_acc.data)?;
    let source_ata_pk =
        get_associated_token_address_with_program_id(&authority.pubkey(), mint, &token_program);

    let destination_ata_pk =
        get_associated_token_address_with_program_id(recipient, mint, &token_program);

    let create_ixn = create_associated_token_account_idempotent(
        &payer.pubkey(),
        recipient,
        mint,
        &token_program,
    );
    let transfer_ixn = spl_token_2022::instruction::transfer_checked(
        &token_program,
        &source_ata_pk,
        mint,
        &destination_ata_pk,
        &authority.pubkey(),
        &[&authority.pubkey()],
        amount,
        mint_state.base.decimals,
    )?;

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_ixn, transfer_ixn],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        svm.latest_blockhash(),
    ));
    match tx_result {
        Ok(_res) => {}
        Err(e) => {
            panic!("Transaction errored\n{:?}", e.meta.logs);
        }
    }

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
) -> Result<(), Box<dyn std::error::Error>> {
    let mut account_info = svm.get_account(pubkey).unwrap();

    write_token_amount_in_place(&mut account_info.data, amount)
        .map_err(|e| format!("write amount failed: {e}"))?;

    svm.set_account(*pubkey, account_info)?;
    Ok(())
}

/// Overwrite the `amount` field of a Token/Token-2022 Account in-place.
/// Works for both programs because `amount` is at a stable offset.
fn write_token_amount_in_place(data: &mut [u8], amount: u64) -> Result<(), String> {
    const AMOUNT_OFFSET: usize = 32  // mint
                               + 32; // owner
    let end = AMOUNT_OFFSET + 8;
    if data.len() < end {
        return Err(format!("account data too short: {} < {}", data.len(), end));
    }
    data[AMOUNT_OFFSET..end].copy_from_slice(&amount.to_le_bytes());
    Ok(())
}
