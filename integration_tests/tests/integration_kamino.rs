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
            AccountingAction, AccountingDirection, 
            AccountingEvent, IntegrationConfig, 
            IntegrationState, IntegrationStatus, 
            IntegrationUpdateEvent, KaminoConfig, 
            ReserveStatus, SvmAlmControllerEvent, 
        }, 
        initialize_integration::kamino_lend::create_initialize_kamino_lend_integration_ix, 
        integrations::kamino::{
            derive_obligation_farm_address, 
            derive_reserve_collateral_supply, 
            derive_reserve_liquidity_supply, 
            derive_vanilla_obligation_address
        }, 
        pull::kamino_lend::create_pull_kamino_lend_ix, 
        push::create_push_kamino_lend_ix, 
        sync_integration::create_sync_kamino_lend_ix
    };
    use borsh::BorshDeserialize;
    use crate::{
        assert_contains_controller_cpi_event, helpers::{ 
            constants::{
                KAMINO_FARMS_PROGRAM_ID, 
                KAMINO_LEND_PROGRAM_ID, 
                USDC_TOKEN_MINT_PUBKEY
            }, 
            setup_test_controller, 
            spl::SPL_TOKEN_PROGRAM_ID, 
            TestContext
        }, 
        subs::{
            derive_controller_authority_pda, 
            edit_ata_amount, 
            fetch_integration_account, 
            fetch_kamino_reserve, 
            fetch_reserve_account, 
            get_liquidity_and_lp_amount, 
            get_token_balance_or_zero, 
            initialize_ata, 
            initialize_reserve, 
            refresh_kamino_obligation, 
            refresh_kamino_reserve, 
            set_kamino_reserve_liquidity_available_amount, 
            setup_kamino_state, 
            transfer_tokens, 
            KaminoTestContext, 
            ReserveKeys
        }
    };

    fn setup_env_and_get_init_ix(
        svm: &mut LiteSVM,
        controller_pk: &Pubkey,
        super_authority: &Keypair,
        description: &str,
        status: IntegrationStatus,
        rate_limit_slope: u64,
        rate_limit_max_outflow: u64,
        permit_liquidation: bool,
        kamino_config: &KaminoConfig,
        reserve_farm_collateral: &Pubkey,
        reserve_farm_debt: &Pubkey,
        mint: &Pubkey,
        obligation_id: u8
    ) -> Result<(Instruction, Pubkey, ReserveKeys), Box<dyn std::error::Error>> {
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
            1_000_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the USDC token
        let usdc_reserve_pk = initialize_reserve(
            svm,
            &controller_pk,
            mint, // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            100_000_000_000, // rate_limit_slope
            100_000_000_000, // rate_limit_max_outflow,
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

        let (
            kamino_init_ix, 
            kamino_integration_pk
        ) = create_initialize_kamino_lend_integration_ix(
            &controller_pk,
            &super_authority.pubkey(),
            &super_authority.pubkey(),
            &description,
            status,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &IntegrationConfig::Kamino(kamino_config.clone()),
            reserve_farm_collateral,
            reserve_farm_debt,
            obligation_id,
            &KAMINO_LEND_PROGRAM_ID
        );

        Ok((kamino_init_ix, kamino_integration_pk, usdc_reserve_pk))

    }

    fn get_push_ix(
        svm: &mut LiteSVM,
        controller_pk: &Pubkey,
        super_authority: &Keypair,
        integration_pk: &Pubkey,
        obligation: &Pubkey,
        kamino_config: &KaminoConfig,
        amount: u64,
        scope_prices: &Pubkey,
        reserve_farm_collateral: &Pubkey
    ) -> Result<Instruction, Box<dyn std::error::Error>> {

        // refresh the reserve and the obligation (kamino) 
        refresh_kamino_reserve(
            svm, 
            &super_authority, 
            &kamino_config.reserve, 
            &kamino_config.market, 
            scope_prices,
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
            reserve_farm_collateral,
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
        amount: u64,
        scope_prices: &Pubkey,
        reserve_farm_collateral: &Pubkey
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        // refresh the reserve and the obligation (kamino) 
        refresh_kamino_reserve(
            svm, 
            &super_authority, 
            &kamino_config.reserve, 
            &kamino_config.market, 
            scope_prices,
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
            reserve_farm_collateral,
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
        
        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _
        } = setup_kamino_state(&mut svm, &USDC_TOKEN_MINT_PUBKEY, &USDC_TOKEN_MINT_PUBKEY);

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &lending_market, 
        );
        
        let kamino_config = KaminoConfig { 
            market: lending_market, 
            reserve: reserve_context.kamino_reserve_pk, 
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 95] 
        };

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (kamino_init_ix, integration_pk, _) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            description,
            status,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &kamino_config, 
            &reserve_context.reserve_farm_collateral,
            &reserve_context.reserve_farm_debt,
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
            .send_transaction(tx.clone())
            .map_err(|e| {
                println!("logs: {}", e.meta.pretty_logs());
                e.err.to_string()
            })?;

        let clock = svm.get_sysvar::<Clock>();

        let integration = fetch_integration_account(&svm, &integration_pk)
            .expect("integration should exist")
            .unwrap();

        assert_eq!(integration.controller, controller_pk);
        assert_eq!(integration.status, IntegrationStatus::Active);
        assert_eq!(integration.rate_limit_slope, rate_limit_slope);
        assert_eq!(integration.rate_limit_max_outflow, rate_limit_max_outflow);
        assert_eq!(
            integration.rate_limit_outflow_amount_available,
            rate_limit_max_outflow
        );
        assert_eq!(integration.rate_limit_remainder, 0);
        assert_eq!(integration.permit_liquidation, permit_liquidation);
        assert_eq!(integration.last_refresh_timestamp, clock.unix_timestamp);
        assert_eq!(integration.last_refresh_slot, clock.slot);

        match integration.clone().config {
            IntegrationConfig::Kamino(config) => {
                assert_eq!(config, kamino_config)
            }
            _ => panic!("invalid config"),
        }

        // Assert state was properly set
        let kamino_state = match integration.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        assert_eq!(kamino_state.last_liquidity_value, 0);
        assert_eq!(kamino_state.last_lp_amount, 0);

        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: integration_pk,
            authority: super_authority.pubkey(),
            old_state: None,
            new_state: Some(integration),
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            expected_event
        );

        // assert obligation farm collateral was created
        let obligation_farm_collateral_pk = derive_obligation_farm_address(
            &reserve_context.reserve_farm_collateral, 
            &obligation
        );

        let obligation_farm_collateral = svm.get_account(&obligation_farm_collateral_pk)
            .unwrap();
        assert!(obligation_farm_collateral.owner.eq(&KAMINO_FARMS_PROGRAM_ID));
        assert!(obligation_farm_collateral.data.len() == 920);

        // assert obligation farm debt was created
        let obligation_farm_debt_pk = derive_obligation_farm_address(
            &reserve_context.reserve_farm_debt, 
            &obligation
        );

        let obligation_farm_debt = svm.get_account(&obligation_farm_debt_pk)
            .unwrap();
        assert!(obligation_farm_debt.owner.eq(&KAMINO_FARMS_PROGRAM_ID));
        assert!(obligation_farm_debt.data.len() == 920);



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

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _
        } = setup_kamino_state(&mut svm, &USDC_TOKEN_MINT_PUBKEY, &USDC_TOKEN_MINT_PUBKEY);

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &lending_market, 
        );
        
        let kamino_config = KaminoConfig { 
            market: lending_market, 
            reserve: reserve_context.kamino_reserve_pk, 
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 95] 
        };

        let reserve_liquidity_destination = derive_reserve_liquidity_supply(
            &kamino_config.market, 
            &kamino_config.reserve_liquidity_mint
        );
        let reserve_collateral_destination = derive_reserve_collateral_supply(
            &kamino_config.market, 
            &kamino_config.reserve_liquidity_mint
        );

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (
            kamino_init_ix, 
            integration_pk,
            reserve_keys
        ) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            description,
            status,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &kamino_config, 
            &reserve_context.reserve_farm_collateral,
            &reserve_context.reserve_farm_debt,
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

        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let integration_before = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let balance_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let reserve_liquidity_destination_balance_before = get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_before = get_token_balance_or_zero(&svm, &reserve_collateral_destination);

        let (liquidity_value_before, lp_amount_before) = get_liquidity_and_lp_amount(
            &svm, 
            &kamino_config.reserve, 
            &kamino_config.obligation
        )?;

        let deposited_amount = 100_000_000;
        let push_ix = get_push_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &integration_pk, 
            &obligation, 
            &kamino_config,
            deposited_amount,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone())
            .map_err(|e| {
                println!("logs: {}", e.meta.pretty_logs());
                e.err.to_string()
            })?;

        let reserve_liquidity_destination_balance_after = get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_after = get_token_balance_or_zero(&svm, &reserve_collateral_destination);

        let liquidity_amount_kamino_vault_delta 
            = reserve_liquidity_destination_balance_after - reserve_liquidity_destination_balance_before;
        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        // actual amount deposited in kamino
        let balance_delta = balance_before - balance_after;

        let integration_after = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();

        let (liquidity_value_after, lp_amount_after) = get_liquidity_and_lp_amount(
            &svm, 
            &kamino_config.reserve, 
            &kamino_config.obligation
        )?;

        let liquidity_value_delta = liquidity_value_after - liquidity_value_before;
        let lp_amount_delta = lp_amount_after - lp_amount_before;
        // Assert Integration rate limits adjusted
        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_before.rate_limit_outflow_amount_available - balance_delta
        );

        // Assert Reserve rate limits adjusted
        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_before.rate_limit_outflow_amount_available - balance_delta
        );

        // NOTE: This wont hold true with TransferFees enables,
        // so the assertion needs to be improved when adding Token2022 tests
        // Assert Reserve vault was debited exact amount
        assert_eq!(balance_after, balance_before - liquidity_amount_kamino_vault_delta);

        // Assert kamino's token account received the tokens
        assert_eq!(
            reserve_liquidity_destination_balance_after, 
            reserve_liquidity_destination_balance_before + balance_delta
        );

        // assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        assert_eq!(
            state_after.last_liquidity_value,
            liquidity_value_after
        );
        assert_eq!(
            state_after.last_lp_amount,
            lp_amount_after
        );
        assert_eq!(
            state_after.last_liquidity_value,
            state_before.last_liquidity_value + liquidity_value_delta,
        );
        assert_eq!(
            state_after.last_lp_amount,
            state_before.last_lp_amount + lp_amount_delta,
        );


        // Assert LP Vault balance increased
        assert_eq!(
            reserve_collateral_destination_balance_after, 
            reserve_collateral_destination_balance_before + lp_amount_after
        );

        let lp_delta = lp_amount_after.saturating_sub(lp_amount_before);
        let vault_delta = reserve_collateral_destination_balance_after
            .saturating_sub(reserve_collateral_destination_balance_before);

        assert_eq!(vault_delta, lp_delta);

        // Assert expected accounting events
        let reserve_sync_expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            reserve: Some(reserve_keys.pubkey),
            mint: kamino_config.reserve_liquidity_mint,
            action: AccountingAction::Sync,
            direction: AccountingDirection::Credit,
            // amount deposited into reserve after initialization
            delta: 1_000_000_000
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            reserve_sync_expected_event 
        );

        let integration_credit_expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pk),
            mint: kamino_config.reserve_liquidity_mint,
            reserve: None,
            direction: AccountingDirection::Credit,
            action: AccountingAction::Deposit,
            delta: liquidity_value_delta
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            integration_credit_expected_event 
        );

        let reserve_debit_expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            mint: kamino_config.reserve_liquidity_mint,
            reserve: Some(reserve_keys.pubkey),
            direction: AccountingDirection::Debit,
            action: AccountingAction::Deposit,
            delta: balance_delta
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            reserve_debit_expected_event 
        );

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

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _
        } = setup_kamino_state(&mut svm, &USDC_TOKEN_MINT_PUBKEY, &USDC_TOKEN_MINT_PUBKEY);

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &lending_market, 
        );
        
        let kamino_config = KaminoConfig { 
            market: lending_market, 
            reserve: reserve_context.kamino_reserve_pk, 
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 95] 
        };

        let reserve_liquidity_destination = derive_reserve_liquidity_supply(
            &kamino_config.market, 
            &kamino_config.reserve_liquidity_mint
        );
        let reserve_collateral_destination = derive_reserve_collateral_supply(
            &kamino_config.market, 
            &kamino_config.reserve_liquidity_mint
        );

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (
            kamino_init_ix, 
            integration_pk,
            reserve_keys
        ) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            description,
            status,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &kamino_config, 
            &reserve_context.reserve_farm_collateral,
            &reserve_context.reserve_farm_debt,
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
        
        let push_ix = get_push_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &integration_pk, 
            &obligation, 
            &kamino_config,
            100_000_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral
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

        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let integration_before = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let balance_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let reserve_liquidity_destination_balance_before = get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_before = get_token_balance_or_zero(&svm, &reserve_collateral_destination);

        let (liquidity_value_before, lp_amount_before) = get_liquidity_and_lp_amount(
            &svm, 
            &kamino_config.reserve, 
            &kamino_config.obligation
        )?;

        let pull_ix = get_pull_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &integration_pk, 
            &obligation, 
            &kamino_config, 
            &kamino_config.reserve,
            100_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, pull_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone())
            .map_err(|e| {
                println!("logs: {}", e.meta.pretty_logs());
                e.err.to_string()
            })?;

        let (liquidity_value_after, lp_amount_after) = get_liquidity_and_lp_amount(
            &svm, 
            &kamino_config.reserve, 
            &kamino_config.obligation
        )?;

        let reserve_liquidity_destination_balance_after = get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_after = get_token_balance_or_zero(&svm, &reserve_collateral_destination);
        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        // actual withdrawal amount
        let balance_delta = balance_after - balance_before;

        let liquidity_amount_kamino_vault_delta 
            = reserve_liquidity_destination_balance_before - reserve_liquidity_destination_balance_after;

        let integration_after = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();

        // Assert integration rate limits adjusted
        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_before.rate_limit_outflow_amount_available + balance_delta
        );

        // Assert Reserve rate limits adjusted
        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_before.rate_limit_outflow_amount_available + balance_delta
        );

        // Assert Reserve vault was credited exact amount
        assert_eq!(balance_after, balance_before + liquidity_amount_kamino_vault_delta);

        // Assert kamino's token account balance decreased
        assert_eq!(
            reserve_liquidity_destination_balance_after, 
            reserve_liquidity_destination_balance_before - balance_delta
        );

        let liquidity_value_delta = liquidity_value_before - liquidity_value_after;

        // Assert LP Vault balance decreased
        let lp_amount_delta = lp_amount_before.saturating_sub(lp_amount_after);
        let vault_delta = reserve_collateral_destination_balance_before
            .saturating_sub(reserve_collateral_destination_balance_after);

        assert_eq!(vault_delta, lp_amount_delta);

        // assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        assert_eq!(
            state_after.last_liquidity_value,
            liquidity_value_after
        );
        assert_eq!(
            state_after.last_lp_amount,
            lp_amount_after
        );
        assert_eq!(
            state_after.last_liquidity_value,
            state_before.last_liquidity_value - liquidity_value_delta,
        );
        assert_eq!(
            state_after.last_lp_amount,
            state_before.last_lp_amount - lp_amount_delta,
        );

        // Assert expected accounting events

        // no reserve.sync event since there hasnt been a change in balance since last push ix

        let integration_debit_expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pk),
            mint: kamino_config.reserve_liquidity_mint,
            reserve: None,
            direction: AccountingDirection::Debit,
            action: AccountingAction::Withdrawal,
            delta: liquidity_value_delta,
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            integration_debit_expected_event 
        );

        let reserve_credit_expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            mint: kamino_config.reserve_liquidity_mint,
            reserve: Some(reserve_keys.pubkey),
            direction: AccountingDirection::Credit,
            action: AccountingAction::Withdrawal,
            delta: balance_delta,
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            reserve_credit_expected_event 
        );


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

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context
        } = setup_kamino_state(&mut svm, &USDC_TOKEN_MINT_PUBKEY, &USDC_TOKEN_MINT_PUBKEY);

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &lending_market, 
        );
        
        let kamino_config = KaminoConfig { 
            market: lending_market, 
            reserve: reserve_context.kamino_reserve_pk, 
            reserve_liquidity_mint: USDC_TOKEN_MINT_PUBKEY, 
            obligation, 
            obligation_id, 
            padding: [0; 95] 
        };

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (
            kamino_init_ix, 
            integration_pk,
            reserve_keys
        ) = setup_env_and_get_init_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            description,
            status,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &kamino_config, 
            &reserve_context.reserve_farm_collateral,
            &reserve_context.reserve_farm_debt,
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
        svm.send_transaction(tx.clone()).unwrap();

        // Deposit some amount into kamino so that there is a change in balance when moving slots forward

        let push_ix = get_push_ix(
            &mut svm, 
            &controller_pk, 
            &super_authority, 
            &integration_pk, 
            &obligation, 
            &kamino_config,
            100_000_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        let rewards_ata = get_associated_token_address(
            &controller_authority, 
            &USDC_TOKEN_MINT_PUBKEY
        );

        let reserve_before 
            = fetch_reserve_account(&svm, &reserve_keys.pubkey)?
            .unwrap();

        edit_ata_amount(
            &mut svm, 
            &controller_authority, 
            &kamino_config.reserve_liquidity_mint, 
            100_000_000_000
        )?;

        // increase the liquidity amount available in the kamino reserve
        // in order to trigger liquidity value change event

        let (liquidity_value_before, lp_amount_before) = get_liquidity_and_lp_amount(
            &svm, 
            &kamino_config.reserve, 
            &kamino_config.obligation
        )?;

        let kamino_reserve = fetch_kamino_reserve(
            &svm, 
            &kamino_config.reserve
        )?;
        let new_kamino_reserve_liq_available_amount = kamino_reserve.liquidity.available_amount + 100_000_000_000;
        set_kamino_reserve_liquidity_available_amount(
            &mut svm, 
            &kamino_config.reserve, 
            new_kamino_reserve_liq_available_amount
        )?;

        let (liquidity_value_after, lp_amount_after) = get_liquidity_and_lp_amount(
            &svm, 
            &kamino_config.reserve, 
            &kamino_config.obligation
        )?;

        let integration_before = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk, 
            &integration_pk,
            &super_authority.pubkey(), 
            &kamino_config, 
            &USDC_TOKEN_MINT_PUBKEY, 
            &farms_context.global_config, 
            &reserve_context.reserve_farm_collateral,
            &rewards_ata, 
            &Pubkey::default(), 
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
            .send_transaction(tx.clone())
            .map_err(|e| {
                println!("logs: {}", e.meta.pretty_logs());
                e.err.to_string()
            })?;

        let integration_after = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reserve_after 
            = fetch_reserve_account(&svm, &reserve_keys.pubkey)?
            .unwrap();

        // Assert emitted events 
        
        let liq_value_delta = reserve_after.last_balance.abs_diff(reserve_before.last_balance);
        // assert reserve sync
        let expected_reserve_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            reserve: Some(reserve_keys.pubkey),
            mint: kamino_config.reserve_liquidity_mint,
            action: AccountingAction::Sync,
            delta: liq_value_delta,
            direction: AccountingDirection::Credit,
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            expected_reserve_event 
        );

        // TODO: create a reserve with rewards available
        // current reward mint has no rewards_available and doesnt match reserve mint
        // no event will be emited.

        // assert lp amount didnt change
        assert_eq!(lp_amount_after, lp_amount_before);

        // assert liquidity_value change event was emitted
        let liq_value_delta = liquidity_value_after.abs_diff(liquidity_value_before);
        let expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent { 
            controller: controller_pk, 
            integration: Some(integration_pk),
            reserve: None,
            mint: kamino_config.reserve_liquidity_mint, 
            action: AccountingAction::Sync, 
            delta: liq_value_delta,
            direction: AccountingDirection::Credit
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            expected_event 
        );

        // assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };

        assert_eq!(
            state_before.last_liquidity_value + liq_value_delta,
            state_after.last_liquidity_value
        );

        //assert lp amount did not change
        assert_eq!(
            state_after.last_lp_amount,
            state_before.last_lp_amount
        );

        Ok(())
    }
}