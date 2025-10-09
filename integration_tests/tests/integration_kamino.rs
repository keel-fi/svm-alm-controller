mod helpers;
mod subs;

mod tests {
    use litesvm::LiteSVM;
    use solana_sdk::{
        clock::Clock, compute_budget::ComputeBudgetInstruction, 
        instruction::Instruction, pubkey::Pubkey, 
        signature::Keypair, signer::Signer, transaction::Transaction
    };
    use spl_associated_token_account_client::address::get_associated_token_address;
    use svm_alm_controller_client::{
        generated::types::{
            IntegrationConfig, IntegrationStatus, 
            KaminoConfig, ReserveStatus, UtilizationMarketConfig
        }, 
        initialize_integration::kamino_lend::create_initialize_kamino_lend_integration_ix, 
        pull::kamino_lend::create_pull_kamino_lend_ix, 
        push::create_push_kamino_lend_ix, 
        sync_integration::create_sync_kamino_lend_ix
    };

    use crate::{
        helpers::{ 
            constants::{
                BONK_MINT, KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, 
                KAMINO_MAIN_MARKET, KAMINO_REFERRER_METADATA, 
                KAMINO_USDC_RESERVE, KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT, 
                KAMINO_USDC_RESERVE_BONK_VAULT, KAMINO_USDC_RESERVE_COLLATERAL_MINT, 
                KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY, KAMINO_USDC_RESERVE_FARM_COLLATERAL, 
                KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY, 
                KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, USDC_TOKEN_MINT_PUBKEY
            }, 
            lite_svm::get_account_data_from_json, 
            setup_test_controller, 
            spl::SPL_TOKEN_PROGRAM_ID, 
            TestContext
        }, 
        subs::{
            derive_controller_authority_pda, 
            derive_vanilla_obligation_address, 
            edit_ata_amount, initialize_ata, 
            initialize_reserve, refresh_kamino_obligation, 
            refresh_kamino_reserve, transfer_tokens
        }
    };

    fn setup_env_and_get_init_ix(
        svm: &mut LiteSVM,
        controller_pk: &Pubkey,
        super_authority: &Keypair,
        kamino_config: &KaminoConfig,
        mint: &Pubkey,
        obligation_id: u8
    ) -> Result<(Instruction, Pubkey), Box<dyn std::error::Error>> {
        set_kamino_accounts(svm);

        // Create an ATA for the USDC account
        let _authority_mint_ata = initialize_ata(
            svm,
            &super_authority,
            &super_authority.pubkey(),
            mint,
        )?;

        edit_ata_amount(
            svm,
            &super_authority.pubkey(),
            mint,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the USDC token
        let _usdc_reserve_pk = initialize_reserve(
            svm,
            &controller_pk,
            mint, // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow,
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            svm,
            &super_authority,
            &super_authority,
            mint,
            &controller_authority,
            1_000_000_000,
        )?;

        let clock = svm.get_sysvar::<solana_sdk::sysvar::clock::Clock>();

        let (
            kamino_init_ix, 
            kamino_integration_pk
        ) = create_initialize_kamino_lend_integration_ix(
            &controller_pk,
            &super_authority.pubkey(),
            &super_authority.pubkey(),
            "test",
            IntegrationStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            true,
            &IntegrationConfig::UtilizationMarket(UtilizationMarketConfig::KaminoConfig(kamino_config.clone())),
            clock.slot,
            obligation_id,
            &KAMINO_LEND_PROGRAM_ID
        );

        Ok((kamino_init_ix, kamino_integration_pk))

    }

    fn get_push_ix(
        svm: &mut LiteSVM,
        controller_pk: &Pubkey,
        super_authority: &Keypair,
        integration_pk: &Pubkey,
        obligation: &Pubkey,
        kamino_config: &KaminoConfig,
        amount: u64
    ) -> Result<Instruction, Box<dyn std::error::Error>> {

        // refresh the reserve and the obligation (kamino) 
        refresh_kamino_reserve(
            svm, 
            &super_authority, 
            &kamino_config.reserve, 
            &kamino_config.market, 
            &KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED,
        )?;

        refresh_kamino_obligation(
            svm, 
            super_authority, 
            &kamino_config.market, 
            obligation,
            None
        )?;
        
        let push_ix = create_push_kamino_lend_ix(
            controller_pk, 
            integration_pk, 
            &super_authority.pubkey(), 
            &kamino_config, 
            amount
        );

        Ok(push_ix)
    }

    fn get_pull_ix(
        svm: &mut LiteSVM,
        controller_pk: &Pubkey,
        super_authority: &Keypair,
        integration_pk: &Pubkey,
        obligation: &Pubkey,
        kamino_config: &KaminoConfig,
        reserve: &Pubkey,
        amount: u64
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        // refresh the reserve and the obligation (kamino) 
        refresh_kamino_reserve(
            svm, 
            &super_authority, 
            &kamino_config.reserve, 
            &kamino_config.market, 
            &KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED,
        )?;

        refresh_kamino_obligation(
            svm, 
            super_authority, 
            &kamino_config.market, 
            obligation,
            Some(reserve)
        )?;
        
        let pull_ix = create_pull_kamino_lend_ix(
            &controller_pk, 
            &integration_pk, 
            &super_authority.pubkey(), 
            &kamino_config, 
            amount
        );

        Ok(pull_ix)
    }

    #[test]
    fn test_kamino_init_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let obligation_id = 0;

        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &KAMINO_MAIN_MARKET, 
            &KAMINO_LEND_PROGRAM_ID
        );
        
        let kamino_config = KaminoConfig { 
            market: KAMINO_MAIN_MARKET, 
            reserve: KAMINO_USDC_RESERVE, 
            reserve_farm_collateral: KAMINO_USDC_RESERVE_FARM_COLLATERAL,
            reserve_farm_debt: Pubkey::default(),
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 30] 
        };

        let (kamino_init_ix, _) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &kamino_config, 
            &USDC_TOKEN_MINT_PUBKEY, 
            obligation_id
        ).unwrap();

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

    #[test]
    fn test_kamino_push_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let obligation_id = 0;

        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &KAMINO_MAIN_MARKET, 
            &KAMINO_LEND_PROGRAM_ID
        );
        
        let kamino_config = KaminoConfig { 
            market: KAMINO_MAIN_MARKET, 
            reserve: KAMINO_USDC_RESERVE, 
            reserve_farm_collateral: KAMINO_USDC_RESERVE_FARM_COLLATERAL,
            reserve_farm_debt: Pubkey::default(),
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 30] 
        };

        let (kamino_init_ix, integration_pk) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &kamino_config, 
            &USDC_TOKEN_MINT_PUBKEY, 
            obligation_id
        ).unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm
            .send_transaction(tx.clone())
            .unwrap();

        // advance time to avoid math overflow in kamino refresh calls
        let mut initial_clock = svm.get_sysvar::<Clock>();
        initial_clock.unix_timestamp = 1754682844;
        initial_clock.slot = 358754275;
        svm.set_sysvar::<Clock>(&initial_clock);
        
        let push_ix = get_push_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &integration_pk, 
            &obligation, 
            &kamino_config,
            100_000_000
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone());

        assert!(tx_result.is_ok(), "{:#?}", tx_result.err());

        Ok(())
    }

    #[test]
    fn test_kamino_pull_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let obligation_id = 0;

        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &KAMINO_MAIN_MARKET, 
            &KAMINO_LEND_PROGRAM_ID
        );
        
        let kamino_config = KaminoConfig { 
            market: KAMINO_MAIN_MARKET, 
            reserve: KAMINO_USDC_RESERVE, 
            reserve_farm_collateral: KAMINO_USDC_RESERVE_FARM_COLLATERAL,
            reserve_farm_debt: Pubkey::default(),
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 30] 
        };

        let (kamino_init_ix, integration_pk) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &kamino_config, 
            &USDC_TOKEN_MINT_PUBKEY, 
            obligation_id
        ).unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm
            .send_transaction(tx.clone())
            .unwrap();

        // advance time to avoid math overflow in kamino refresh calls
        let mut initial_clock = svm.get_sysvar::<Clock>();
        initial_clock.unix_timestamp = 1754682844;
        initial_clock.slot = 358754275;
        svm.set_sysvar::<Clock>(&initial_clock);
        
        let push_ix = get_push_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &integration_pk, 
            &obligation, 
            &kamino_config,
            100_000_000
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm
            .send_transaction(tx)
            .unwrap();

        svm.expire_blockhash();

        let pull_ix = get_pull_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &integration_pk, 
            &obligation, 
            &kamino_config, 
            &kamino_config.reserve,
            100_000
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, pull_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone());

        assert!(tx_result.is_ok(), "{:#?}", tx_result.err());

        Ok(())
    }

    #[test]
    fn test_kamino_sync_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let obligation_id = 0;

        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &KAMINO_MAIN_MARKET, 
            &KAMINO_LEND_PROGRAM_ID
        );
        
        let kamino_config = KaminoConfig { 
            market: KAMINO_MAIN_MARKET, 
            reserve: KAMINO_USDC_RESERVE, 
            reserve_farm_collateral: KAMINO_USDC_RESERVE_FARM_COLLATERAL,
            reserve_farm_debt: Pubkey::default(),
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 30] 
        };

        let (kamino_init_ix, integration_pk) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &kamino_config, 
            &USDC_TOKEN_MINT_PUBKEY, 
            obligation_id
        ).unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm
            .send_transaction(tx.clone())
            .unwrap();

        let rewards_ata = get_associated_token_address(
            &controller_authority, 
            &BONK_MINT
        );

        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk, 
            &integration_pk,
            &super_authority.pubkey(), 
            &kamino_config, 
            &BONK_MINT, 
            &KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, 
            &rewards_ata, 
            &KAMINO_FARMS_PROGRAM_ID, 
            &SPL_TOKEN_PROGRAM_ID
        );
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, sync_ix],
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