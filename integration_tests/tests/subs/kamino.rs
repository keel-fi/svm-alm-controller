#![allow(dead_code)]
use std::{error::Error, u64};

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account, program_pack::Pack, pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction
};
use spl_token::state::{Mint, Account as TokenAccount};
use svm_alm_controller_client::{
    create_refresh_kamino_obligation_instruction, 
    create_refresh_kamino_reserve_instruction, integrations::kamino::{
        derive_farm_vaults_authority, derive_kfarms_treasury_vault_authority, derive_market_authority_address, derive_reserve_collateral_mint, derive_reserve_collateral_supply, derive_reserve_liquidity_supply, derive_rewards_treasury_vault, derive_rewards_vault
    }, 
};

use crate::helpers::{
    constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID}, 
    kamino::{math_utils::Fraction, state::{kfarms::{FarmState, GlobalConfig, RewardInfo}, klend::{KaminoReserve, LendingMarket, Obligation}}}
};

pub fn get_liquidity_and_lp_amount(
    svm: &LiteSVM,
    kamino_reserve_pk: &Pubkey,
    obligation_pk: &Pubkey,
) -> Result<(u64, u64), Box<dyn std::error::Error>> {
    let obligation_acc = svm.get_account(obligation_pk)
        .expect("could not get obligation");

    let obligation_state = Obligation::try_from(&obligation_acc.data)?;

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
        let kamino_reserve_state = KaminoReserve::try_from(&kamino_reserve_acc.data)?;
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

    let kamino_reserve = KaminoReserve::try_from(&acc.data)?.clone();

    Ok(kamino_reserve)
}

pub fn set_kamino_reserve_liquidity_available_amount(
    svm: &mut LiteSVM,
    kamino_reserve_pk: &Pubkey,
    amount: u64
) -> Result<(), Box<dyn std::error::Error>> {
    let acc = svm.get_account(kamino_reserve_pk)
        .expect("failed to get kamino reserve ");
    let mut state = KaminoReserve::try_from(&acc.data)?.clone();
    state.liquidity.available_amount = amount;

    let mut state_data = Vec::with_capacity(std::mem::size_of::<KaminoReserve>() + 8);
    state_data.extend_from_slice(&KaminoReserve::DISCRIMINATOR);
    state_data.extend_from_slice(&bytemuck::bytes_of(&state));

    svm.set_account(*kamino_reserve_pk, Account {
        data : state_data,
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

pub struct KaminoFarmsContext {
    pub global_config: Pubkey,
}

pub struct KaminoReserveContext {
    pub kamino_reserve_pk: Pubkey,
    pub liquidity_supply_vault: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_collateral_supply: Pubkey,
    pub reserve_farm_collateral: Pubkey,
    pub reserve_farm_debt: Pubkey,
}
pub struct KaminoTestContext {
    pub lending_market: Pubkey,
    pub reserve_context: KaminoReserveContext,
    pub farms_context: KaminoFarmsContext
}

/// sets all account required by kamino integration
/// mint decimals are set to 6 for simplicity
pub fn setup_kamino_state(
    svm: &mut LiteSVM,
    liquidity_mint: &Pubkey,
    reward_mint: &Pubkey,
) -> KaminoTestContext {

    // setup lending market (klend)

    let lending_market_pk = Pubkey::new_unique();
    let mut market = LendingMarket::default();
    let (
        lending_market_authority, 
        market_auth_bump
    ) = derive_market_authority_address(&lending_market_pk);
    // the bump seed is checked during user initialization, and the default is 0
    market.bump_seed = market_auth_bump as u64;
    market.price_refresh_trigger_to_max_age_pct = 1;
    svm.set_account(
        lending_market_pk, 
        Account { 
            lamports: u64::MAX, 
            data: vec![
                LendingMarket::DISCRIMINATOR.to_vec(),
                bytemuck::bytes_of(&market).to_vec()
            ].concat(), 
            owner: KAMINO_LEND_PROGRAM_ID, 
            executable: false, 
            rent_epoch: u64::MAX 
        }
    )
    .unwrap();


    // setup global config (kfarms)

    let global_config_pk = Pubkey::new_unique();
    let mut global_config = GlobalConfig::default();
    // get the treasury vault (fees) and store it in config
    let treasury_vault = derive_rewards_treasury_vault(
        &global_config_pk, 
        reward_mint
    );
    let (
        treasury_vault_authority, 
        treasury_vault_authority_bump
    ) = derive_kfarms_treasury_vault_authority(&global_config_pk);
    // create the treasury vault
    set_token_account(
        svm, 
        &treasury_vault, 
        &treasury_vault_authority, 
        reward_mint, 
        0
    );
    global_config.treasury_vaults_authority = treasury_vault_authority;
    global_config.treasury_vaults_authority_bump = treasury_vault_authority_bump as u64;
    svm.set_account(
        global_config_pk, 
        Account { 
            lamports: u64::MAX, 
            data: vec![
                GlobalConfig::DISCRIMINATOR.to_vec(),
                bytemuck::bytes_of(&global_config).to_vec()
            ].concat(), 
            owner: KAMINO_FARMS_PROGRAM_ID, 
            executable: false, 
            rent_epoch: u64::MAX 
        }
    )
    .unwrap();


    // setup reserve_farm_collateral (kfarms)

    let reserve_farm_collateral = Pubkey::new_unique();
    let mut farm_collateral = FarmState::default();
    farm_collateral.global_config = global_config_pk;
    // we make the farm delegated, must be the lending market authority
    // the PDA signing the CPI into KFARMS
    farm_collateral.delegate_authority = lending_market_authority;
    farm_collateral.scope_oracle_price_id = u64::MAX;
    farm_collateral.num_reward_tokens = u64::MAX;
    farm_collateral.num_reward_tokens = 1;
    // set reward info for harvesting rewards
    // create the farm vault
    let reward_vault = derive_rewards_vault(
        &reserve_farm_collateral, 
        &reward_mint
    );
    let farm_vault_authority = derive_farm_vaults_authority(
        &reserve_farm_collateral
    );
    set_token_account(
        svm, 
        &reward_vault, 
        &farm_vault_authority, 
        reward_mint, 
        u64::MAX
    );

    let mut reward_info = RewardInfo::default();
    reward_info.token.decimals = 6;
    reward_info.token.mint = *reward_mint;
    reward_info.token.token_program= spl_token::ID;

    farm_collateral.reward_infos[0] = reward_info;
    svm.set_account(
        reserve_farm_collateral, 
        Account { 
            lamports: u64::MAX, 
            data: vec![
                FarmState::DISCRIMINATOR.to_vec(),
                bytemuck::bytes_of(&farm_collateral).to_vec()
            ].concat(), 
            owner: KAMINO_FARMS_PROGRAM_ID, 
            executable: false, 
            rent_epoch: u64::MAX 
        }
    )
    .unwrap();


    // set reserve_farm_debt (use TBD) (kfarms)

    let reserve_farm_debt = Pubkey::new_unique();
    let mut farm_debt = FarmState::default();
    farm_debt.global_config = global_config_pk;
    farm_debt.delegate_authority = lending_market_authority;
    farm_debt.scope_oracle_price_id = u64::MAX;
    svm.set_account(
        reserve_farm_debt, 
        Account { 
            lamports: u64::MAX, 
            data: vec![
                FarmState::DISCRIMINATOR.to_vec(),
                bytemuck::bytes_of(&farm_debt).to_vec()
            ].concat(), 
            owner: KAMINO_FARMS_PROGRAM_ID, 
            executable: false, 
            rent_epoch: u64::MAX 
        }
    )
    .unwrap();



    // setup reserve (klend)

    let kamino_reserve_pk = Pubkey::new_unique();
    let mut kamino_reserve = KaminoReserve::default();
    kamino_reserve.lending_market = lending_market_pk;
    kamino_reserve.liquidity.mint_pubkey = *liquidity_mint;
    kamino_reserve.liquidity.mint_decimals = 6;
    kamino_reserve.liquidity.market_price_sf = Fraction::ONE.to_bits();
    kamino_reserve.farm_collateral = reserve_farm_collateral;
    kamino_reserve.farm_debt = reserve_farm_debt;
    kamino_reserve.version = 1;
    // make the reserve max_age_price_seconds
    // high so that we dont need an oracle to update price
    kamino_reserve.config.token_info.max_age_price_seconds = u64::MAX;
    // increase deposit limit
    kamino_reserve.config.deposit_limit = u64::MAX;

    let liquidity_supply_vault = derive_reserve_liquidity_supply(
        &lending_market_pk, 
        &liquidity_mint
    );
    set_token_account(
        svm, 
        &liquidity_supply_vault, 
        &lending_market_authority, 
        liquidity_mint, 
        0
    );

    let reserve_collateral_mint = derive_reserve_collateral_mint(
        &lending_market_pk, 
        &liquidity_mint
    );
    set_mint(
        svm, 
        &reserve_collateral_mint, 
        &lending_market_authority, 
        0
    );

    let reserve_collateral_supply = derive_reserve_collateral_supply(
        &lending_market_pk, 
        &liquidity_mint
    );
    set_token_account(
        svm, 
        &reserve_collateral_supply, 
        &lending_market_authority, 
        &reserve_collateral_mint, 
        0
    );

    kamino_reserve.liquidity.supply_vault = liquidity_supply_vault;
    kamino_reserve.collateral.mint_pubkey = reserve_collateral_mint;
    kamino_reserve.collateral.supply_vault = reserve_collateral_supply;
    svm.set_account(
        kamino_reserve_pk, 
        Account { 
            lamports: u64::MAX, 
            data: vec![
                KaminoReserve::DISCRIMINATOR.to_vec(),
                bytemuck::bytes_of(&kamino_reserve).to_vec()
            ].concat(), 
            owner: KAMINO_LEND_PROGRAM_ID, 
            executable: false, 
            rent_epoch: u64::MAX 
        }
    )
    .unwrap();


    let reserve_context = KaminoReserveContext {
        kamino_reserve_pk,
        liquidity_supply_vault,
        reserve_collateral_mint,
        reserve_collateral_supply,
        reserve_farm_collateral,
        reserve_farm_debt
    };

    let farms_context = KaminoFarmsContext {
        global_config: global_config_pk
    };
    
    KaminoTestContext {
        lending_market: lending_market_pk,
        reserve_context,
        farms_context
    }
}

fn set_mint(
    svm: &mut LiteSVM,
    mint_pk: &Pubkey,
    mint_authority: &Pubkey,
    supply: u64,
) {
    let mint_state = Mint {
        mint_authority: Some(*mint_authority).into(),
        supply,
        decimals: 6,
        is_initialized: true,
        freeze_authority: None.into(),
    };

    let mut data = vec![0u8; Mint::LEN];
    Mint::pack(mint_state, &mut data).expect("failed to pack Mint");

    svm.set_account(*mint_pk, Account {
        lamports: u64::MAX,
        data: data,
        owner: spl_token::id(),
        executable: false,
        rent_epoch: u64::MAX,
    }).expect("failed to set mint");
}

fn set_token_account(
    svm: &mut LiteSVM,
    token_account_pk: &Pubkey,
    owner: &Pubkey,
    mint_pk: &Pubkey,
    amount: u64,
) {
    let token_account = TokenAccount {
        mint: *mint_pk,
        owner: *owner,
        amount,
        delegate: None.into(),
        state: spl_token::state::AccountState::Initialized,
        is_native: None.into(),
        delegated_amount: 0,
        close_authority: None.into(),
    };

    let mut data = vec![0u8; TokenAccount::LEN];
    TokenAccount::pack(token_account, &mut data).expect("failed to pack token account");

    svm.set_account(
        *token_account_pk,
        Account {
            lamports: u64::MAX,
            data,
            owner: spl_token::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .expect("failed to set token account");
}