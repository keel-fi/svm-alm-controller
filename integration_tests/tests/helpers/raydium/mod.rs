pub mod instruction;
pub mod state;

use std::mem;
use std::ops::Mul;

use bytemuck::Zeroable;
pub use instruction::*;
use litesvm::LiteSVM;
use solana_sdk::account::{Account, ReadableAccount};
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey;
use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
pub use state::*;

use super::spl::{
    setup_token_account, setup_token_mint, NATIVE_MINT_ADDRESS, SPL_TOKEN_PROGRAM_ID,
};

pub const RAYDIUM_LEGACY_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
pub const RAYDIUM_LEGACY_AMM_V4_UPGRADE_AUTH: Pubkey =
    pubkey!("GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ");

pub struct AmmAccounts {
    pub amm: Pubkey,
    pub amm_authority: Pubkey,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
    pub open_orders: Pubkey,
    pub market_program: Pubkey,
    pub market: Pubkey,
    pub market_bids: Pubkey,
    pub market_asks: Pubkey,
    pub market_event_queue: Pubkey,
    pub market_coin_vault: Pubkey,
    pub market_pc_vault: Pubkey,
    pub market_vault_signer: Pubkey,
}

pub fn setup_amm_config(svm: &mut LiteSVM) -> Pubkey {
    let amm_pubkey = Pubkey::new_unique();
    let amm_config = AmmConfig {
        // shouldn't matter for our use case.
        pnl_owner: Pubkey::new_unique(),
        cancel_owner: Pubkey::new_unique(),
        pending_1: [0u64; 28],
        pending_2: [0u64; 31],
        create_pool_fee: 0,
    };
    let space = mem::size_of::<AmmConfig>();
    let rent = svm.minimum_balance_for_rent_exemption(space);
    let mut account = AccountSharedData::new(rent, space, &RAYDIUM_LEGACY_AMM_V4);
    account.set_data_from_slice(&amm_config.to_bytes());
    svm.set_account(amm_pubkey, Account::from(account)).unwrap();
    amm_pubkey
}

pub fn setup_amm(
    mut svm: &mut LiteSVM,
    coin_mint_address: Pubkey,
    pc_mint_address: Pubkey,
    coin_liquidity_amount: u64,
    pc_liquidity_amount: u64,
) -> AmmAccounts {
    let amm_info_pubkey = Pubkey::new_unique();
    // Fake OpenBook market accounts
    let market = Pubkey::new_unique();
    let open_orders = Pubkey::new_unique();
    let market_program = Pubkey::new_unique();
    let target_orders = Pubkey::new_unique();

    let (amm_authority, amm_nonce) =
        Pubkey::find_program_address(&[b"amm authority"], &RAYDIUM_LEGACY_AMM_V4);
    // Read token info for AMM state
    let coin_acct = svm.get_account(&coin_mint_address).unwrap();
    let coin_mint = spl_token_2022::state::Mint::unpack(coin_acct.data()).unwrap();
    let pc_acct = svm.get_account(&pc_mint_address).unwrap();
    let pc_mint = spl_token_2022::state::Mint::unpack(pc_acct.data()).unwrap();
    let (coin_vault_address, _bump_seed) = Pubkey::find_program_address(
        &[
            &RAYDIUM_LEGACY_AMM_V4.to_bytes(),
            &market.to_bytes(),
            // COIN_VAULT_ASSOCIATED_SEED
            b"coin_vault_associated_seed",
        ],
        &RAYDIUM_LEGACY_AMM_V4,
    );
    let coin_native = if coin_mint_address.eq(&NATIVE_MINT_ADDRESS) {
        Some(1)
    } else {
        None
    };
    setup_token_account(
        &mut svm,
        &coin_vault_address,
        &coin_mint_address,
        &amm_authority,
        coin_liquidity_amount,
        &SPL_TOKEN_PROGRAM_ID,
        coin_native,
    );
    let (pc_vault_address, _bump_seed) = Pubkey::find_program_address(
        &[
            &RAYDIUM_LEGACY_AMM_V4.to_bytes(),
            &market.to_bytes(),
            // PC_VAULT_ASSOCIATED_SEED
            b"pc_vault_associated_seed",
        ],
        &RAYDIUM_LEGACY_AMM_V4,
    );
    let pc_native = if pc_mint_address.eq(&NATIVE_MINT_ADDRESS) {
        Some(1)
    } else {
        None
    };
    setup_token_account(
        &mut svm,
        &pc_vault_address,
        &pc_mint_address,
        &amm_authority,
        pc_liquidity_amount,
        &SPL_TOKEN_PROGRAM_ID,
        pc_native,
    );

    // Create an LP Mint account
    let (lp_mint_address, _bump_seed) = Pubkey::find_program_address(
        &[
            &RAYDIUM_LEGACY_AMM_V4.to_bytes(),
            &market.to_bytes(),
            // LP_MINT_ASSOCIATED_SEED
            b"lp_mint_associated_seed",
        ],
        &RAYDIUM_LEGACY_AMM_V4,
    );
    // Uses the Coin's decimals, same as Raydium Initialize2 instruction.
    setup_token_mint(
        &mut svm,
        &lp_mint_address,
        coin_mint.decimals,
        &amm_authority,
    );

    let sys_decimal_value = if pc_mint.decimals > coin_mint.decimals {
        (10 as u64).pow(pc_mint.decimals as u32)
    } else {
        (10 as u64).pow(coin_mint.decimals as u32)
    };
    let amm = AmmInfo {
        fees: Fees::initialize(),
        state_data: StateData::initialize(0),
        // Status 6, the status all*
        status: AmmStatus::SwapOnly.into_u64(),
        nonce: amm_nonce as u64,
        order_num: 0,
        depth: 0,
        coin_decimals: u64::from(coin_mint.decimals),
        pc_decimals: u64::from(pc_mint.decimals),
        state: AmmState::IdleState.into_u64(),
        reset_flag: AmmResetFlag::ResetNo.into_u64(),
        // Not sure whether to mark this as 0
        min_size: 1,
        vol_max_cut_ratio: 500,
        amount_wave: sys_decimal_value.mul(5).checked_div(1000).unwrap(),
        coin_lot_size: 1,
        pc_lot_size: 1,
        min_price_multiplier: 1,
        max_price_multiplier: 1_000_000_000,
        sys_decimal_value,
        client_order_id: 0,
        padding1: Zeroable::zeroed(),
        recent_epoch: 0,
        padding2: Zeroable::zeroed(),
        amm_owner: RAYDIUM_LEGACY_AMM_V4_UPGRADE_AUTH,
        /* Token related state */
        coin_vault_mint: coin_mint_address,
        coin_vault: coin_vault_address,
        pc_vault_mint: pc_mint_address,
        pc_vault: pc_vault_address,
        lp_mint: lp_mint_address,
        lp_amount: 0,
        /* OpenBook market state */
        open_orders,
        market,
        market_program,
        target_orders,
    };
    let space = mem::size_of::<AmmInfo>();
    let rent = svm.minimum_balance_for_rent_exemption(space);
    let mut account = AccountSharedData::new(rent, space, &RAYDIUM_LEGACY_AMM_V4);
    account.set_data_from_slice(&amm.to_bytes());
    svm.set_account(amm_info_pubkey, Account::from(account))
        .unwrap();

    AmmAccounts {
        amm: amm_info_pubkey,
        amm_authority,
        coin_vault: coin_vault_address,
        pc_vault: pc_vault_address,
        market_program,
        market,
        open_orders,
        market_asks: Pubkey::new_unique(),
        market_bids: Pubkey::new_unique(),
        market_coin_vault: Pubkey::new_unique(),
        market_event_queue: Pubkey::new_unique(),
        market_pc_vault: Pubkey::new_unique(),
        market_vault_signer: Pubkey::new_unique(),
    }
}
