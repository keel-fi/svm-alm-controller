use litesvm::LiteSVM;
use solana_sdk::account::Account;
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey;
use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};

pub const SPL_TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const NATIVE_MINT_ADDRESS: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// Sets the state of an SPL Token Account in a given address.
#[deprecated(
    note = "Please use initialize_ata and mint_tokens as they handle complex Token2022 logic"
)]
pub fn setup_token_account(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
    token_program: &Pubkey,
    is_native: Option<u64>,
) {
    let token_account = spl_token_2022::state::Account {
        mint: *mint,
        owner: *owner,
        amount,
        delegate: None.into(),
        state: spl_token_2022::state::AccountState::Initialized.into(),
        is_native: is_native.into(),
        delegated_amount: 0,
        close_authority: None.into(),
    };

    let space = spl_token_2022::state::Account::LEN;
    let rent = svm.minimum_balance_for_rent_exemption(space);

    let mut lamports = rent;
    if is_native.is_some() {
        // adjust lamport balance when dealing with WSol
        lamports += amount;
    }
    let mut account = AccountSharedData::new(lamports, space, token_program);
    let mut data = [0u8; spl_token_2022::state::Account::LEN];
    spl_token_2022::state::Account::pack(token_account, &mut data).unwrap();
    account.set_data_from_slice(&data);
    svm.set_account(*pubkey, Account::from(account)).unwrap();
}

/// Sets the state of an SPL Token Mint in a given address.
#[deprecated(note = "Please use initialize_mint as it handles complex Token2022 logic")]
pub fn setup_token_mint(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    decimals: u8,
    mint_authority: &Pubkey,
    token_program: &Pubkey,
) {
    let mint = spl_token_2022::state::Mint {
        mint_authority: Some(*mint_authority).into(),
        supply: 0,
        decimals,
        is_initialized: true,
        freeze_authority: None.into(),
    };
    let space = spl_token_2022::state::Mint::LEN;
    let rent = svm.minimum_balance_for_rent_exemption(space);
    let mut account = AccountSharedData::new(rent, space, token_program);
    let mut data = [0u8; spl_token_2022::state::Mint::LEN];
    spl_token_2022::state::Mint::pack(mint, &mut data).unwrap();
    account.set_data_from_slice(&data);
    svm.set_account(*pubkey, Account::from(account)).unwrap();
}
