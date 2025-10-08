mod helpers;
mod subs;

mod tests {
    use litesvm::LiteSVM;
    use solana_sdk::{compute_budget::ComputeBudgetInstruction, pubkey::Pubkey, signer::Signer, transaction::Transaction};
    use svm_alm_controller_client::{generated::types::{IntegrationConfig, IntegrationStatus, KaminoConfig, ReserveStatus, UtilizationMarketConfig}, initialize_integration::kamino_lend::create_initialize_kamino_lend_integration_ix, SVM_ALM_CONTROLLER_ID};

    use crate::{helpers::{
        constants::{
            BONK_MINT, KAMINO_LEND_PROGRAM_ID, KAMINO_MAIN_MARKET, KAMINO_REFERRER_METADATA, KAMINO_USDC_RESERVE, KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT, KAMINO_USDC_RESERVE_BONK_VAULT, KAMINO_USDC_RESERVE_COLLATERAL_MINT, KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY, KAMINO_USDC_RESERVE_FARM_COLLATERAL, KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY, KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, USDC_TOKEN_MINT_PUBKEY
        }, 
        lite_svm::get_account_data_from_json, setup_test_controller, TestContext
    }, subs::{derive_controller_authority_pda, derive_vanilla_obligation_address, edit_ata_amount, initialize_ata, initialize_reserve, transfer_tokens}};

    #[test]
    fn kamino_init_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        set_kamino_accounts(&mut svm);
        let usdc_mint = USDC_TOKEN_MINT_PUBKEY;
        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &usdc_mint,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &usdc_mint,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the USDC token
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &usdc_mint, // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow,
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &usdc_mint,
            &controller_authority,
            1_000_000_000,
        )?;

        let market = KAMINO_MAIN_MARKET;
        let reserve = KAMINO_USDC_RESERVE;
        let reserve_farm_collateral = KAMINO_USDC_RESERVE_FARM_COLLATERAL;
        let reserve_farm_debt = Pubkey::default();

        let obligation_id = 0;
        // Initialize a kamino main market USDC Integration
        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &market, 
            &KAMINO_LEND_PROGRAM_ID
        );
        
        let kamino_config = KaminoConfig { 
            market, 
            reserve, 
            reserve_farm_collateral,
            reserve_farm_debt,
            reserve_liquidity_mint: usdc_mint, 
            obligation, 
            obligation_id, 
            padding: [0; 30] 
        };

        let clock = svm.get_sysvar::<solana_sdk::sysvar::clock::Clock>();

        let (
            kamino_init_ix, 
            _kamino_integration_pk
        ) = create_initialize_kamino_lend_integration_ix(
            &controller_pk,
            &super_authority.pubkey(),
            &super_authority.pubkey(),
            "test",
            IntegrationStatus::Active,
            10000,
            10000,
            true,
            &IntegrationConfig::UtilizationMarket(UtilizationMarketConfig::KaminoConfig(kamino_config)),
            clock.slot,
            obligation_id,
            &KAMINO_LEND_PROGRAM_ID
        );

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone());

        assert!(tx_result.is_ok(), "{:#?}", tx_result.err());
        
        Ok(())
    }


    fn set_kamino_accounts(svm: &mut LiteSVM) {
        let kamino_main_market_account = get_account_data_from_json("./fixtures/kamino_main_market.json");
        svm.set_account(KAMINO_MAIN_MARKET, kamino_main_market_account)
            .unwrap();
        let kamino_usdc_reserve = get_account_data_from_json("./fixtures/kamino_usdc_reserve.json");
        svm.set_account(KAMINO_USDC_RESERVE, kamino_usdc_reserve)
            .unwrap();
        let kamino_usdc_reserve_farm_collateral = get_account_data_from_json("./fixtures/usdc_reserve_farm_collateral.json");
        svm.set_account(KAMINO_USDC_RESERVE_FARM_COLLATERAL, kamino_usdc_reserve_farm_collateral)
            .unwrap();
        let kamino_referrer_user_metadata = get_account_data_from_json("./fixtures/kamino_referrer_metadata.json");
        svm.set_account(KAMINO_REFERRER_METADATA, kamino_referrer_user_metadata)
            .unwrap();
        let kamino_usdc_reserve_liquidity_supply = get_account_data_from_json("./fixtures/kamino_usdc_reserve_liquidity_supply.json");
        svm.set_account(KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY, kamino_usdc_reserve_liquidity_supply)
            .unwrap();
        let kamino_usdc_reserve_collateral_mint = get_account_data_from_json("./fixtures/kamino_usdc_reserve_collateral_mint.json");
        svm.set_account(KAMINO_USDC_RESERVE_COLLATERAL_MINT, kamino_usdc_reserve_collateral_mint)
            .unwrap();
        let kamino_usdc_reserve_collateral_supply = get_account_data_from_json("./fixtures/kamino_usdc_reserve_collateral_supply.json");
        svm.set_account(KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY, kamino_usdc_reserve_collateral_supply)
            .unwrap();
        let kamino_usdc_reserve_scope_config_price_feed = get_account_data_from_json("./fixtures/kamino_usdc_reserve_scope_config_price_feed.json");
        svm.set_account(KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, kamino_usdc_reserve_scope_config_price_feed)
            .unwrap();
        let kamino_usdc_reserve_farm_global_config = get_account_data_from_json("./fixtures/kamino_farm_global_config.json");
        svm.set_account(KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, kamino_usdc_reserve_farm_global_config)
            .unwrap();
        let bonk_mint = get_account_data_from_json("./fixtures/bonk_mint.json");
        svm.set_account(BONK_MINT, bonk_mint)
            .unwrap();
        let bonk_reward_vault = get_account_data_from_json("./fixtures/usdc_reserve_bonk_vault.json");
        svm.set_account(KAMINO_USDC_RESERVE_BONK_VAULT, bonk_reward_vault)
            .unwrap();
        let bonk_treasury_vaut = get_account_data_from_json("./fixtures/usdc_reserve_bonk_treasury_vault.json");
        svm.set_account(KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT, bonk_treasury_vaut)
            .unwrap();
    }
}