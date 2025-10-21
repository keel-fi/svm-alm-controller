mod helpers;
mod subs;

mod tests {
    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{
            constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, USDC_TOKEN_MINT_PUBKEY},
            kamino::state::klend::Obligation,
            setup_test_controller,
            spl::SPL_TOKEN_PROGRAM_ID,
            TestContext,
        },
        subs::{
            airdrop_lamports, derive_controller_authority_pda, edit_ata_amount,
            fetch_integration_account, fetch_kamino_obligation, fetch_kamino_reserve,
            fetch_reserve_account, get_liquidity_and_lp_amount, get_token_balance_or_zero,
            initialize_ata, initialize_mint, initialize_reserve, manage_permission,
            refresh_kamino_obligation, refresh_kamino_reserve,
            set_kamino_reserve_liquidity_available_amount,
            set_obligation_farm_rewards_issued_unclaimed, setup_additional_reserves,
            setup_kamino_state, transfer_tokens, KaminoTestContext, ReserveKeys,
        },
    };
    use borsh::BorshDeserialize;
    use litesvm::LiteSVM;
    use solana_sdk::{
        clock::Clock,
        compute_budget::ComputeBudgetInstruction,
        instruction::{Instruction, InstructionError},
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use svm_alm_controller_client::{
        generated::{
            accounts::Integration,
            types::{
                AccountingAction, AccountingDirection, AccountingEvent, IntegrationConfig,
                IntegrationState, IntegrationStatus, IntegrationUpdateEvent, KaminoConfig,
                PermissionStatus, ReserveStatus, SvmAlmControllerEvent,
            },
        },
        initialize_integration::kamino_lend::create_initialize_kamino_lend_integration_ix,
        integrations::kamino::{
            derive_obligation_farm_address, derive_reserve_collateral_supply,
            derive_reserve_liquidity_supply, derive_vanilla_obligation_address,
        },
        pull::kamino_lend::create_pull_kamino_lend_ix,
        push::create_push_kamino_lend_ix,
        sync_integration::create_sync_kamino_lend_ix,
    };
    use test_case::test_case;

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
        obligation_id: u8,
        integration_reserve_token_program: &Pubkey,
    ) -> Result<(Instruction, Pubkey, ReserveKeys), Box<dyn std::error::Error>> {
        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the given mint
        let reserve_pk = create_and_fund_controller_reserves(
            svm,
            controller_pk,
            super_authority,
            &controller_authority,
            mint,
            integration_reserve_token_program,
        )?;

        let (kamino_init_ix, kamino_integration_pk) = create_initialize_kamino_lend_integration_ix(
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
            &KAMINO_LEND_PROGRAM_ID,
        );

        Ok((kamino_init_ix, kamino_integration_pk, reserve_pk))
    }

    fn create_and_fund_controller_reserves(
        svm: &mut LiteSVM,
        controller_pk: &Pubkey,
        super_authority: &Keypair,
        controller_authority: &Pubkey,
        mint: &Pubkey,
        reserve_token_program: &Pubkey,
    ) -> Result<ReserveKeys, Box<dyn std::error::Error>> {
        let _authority_mint_ata =
            initialize_ata(svm, &super_authority, &super_authority.pubkey(), mint)?;

        edit_ata_amount(svm, &super_authority.pubkey(), mint, 1_000_000_000_000).map_err(|e| {
            println!("edit_ata_amount error: {}", e);
            e
        })?;

        let reserve_keys = initialize_reserve(
            svm,
            &controller_pk,
            mint,             // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            100_000_000_000, // rate_limit_slope
            100_000_000_000, // rate_limit_max_outflow,
            reserve_token_program,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            svm,
            &super_authority,
            &super_authority,
            mint,
            &controller_authority,
            1_000_000_000_000,
        )?;

        Ok(reserve_keys)
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
        kamino_reserve_farm_collateral: &Pubkey,
        reserve_vault_token_program: &Pubkey,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        // fetch and deserialize the obligation to get the active deposits
        // that need to be passed to refresh_kamino_obligation
        let obligation_acc = svm
            .get_account(obligation)
            .expect("Failed to fetch obligation");

        let obligation_state = Obligation::try_from(&obligation_acc.data)?;
        let obligation_reserves: Vec<&Pubkey> = obligation_state
            .deposits
            .iter()
            .filter_map(|deposit| {
                if deposit.deposit_reserve != Pubkey::default() {
                    return Some(&deposit.deposit_reserve);
                }
                None
            })
            .collect();

        // refresh the reserve and the obligation (kamino)
        for reserve in &obligation_reserves {
            refresh_kamino_reserve(
                svm,
                &super_authority,
                reserve,
                &kamino_config.market,
                scope_prices,
            )?;
        }

        refresh_kamino_obligation(
            svm,
            super_authority,
            &kamino_config.market,
            obligation,
            obligation_reserves,
        )?;

        let push_ix = create_push_kamino_lend_ix(
            controller_pk,
            integration_pk,
            &super_authority.pubkey(),
            &kamino_config,
            kamino_reserve_farm_collateral,
            reserve_vault_token_program,
            amount,
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
        amount: u64,
        scope_prices: &Pubkey,
        reserve_farm_collateral: &Pubkey,
        reserve_vault_token_program: &Pubkey,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        let obligation_acc = svm
            .get_account(obligation)
            .expect("Failed to fetch obligation");

        let obligation_state = Obligation::try_from(&obligation_acc.data)?;
        let obligation_reserves: Vec<&Pubkey> = obligation_state
            .deposits
            .iter()
            .filter_map(|deposit| {
                if deposit.deposit_reserve != Pubkey::default() {
                    return Some(&deposit.deposit_reserve);
                }
                None
            })
            .collect();

        // refresh the reserve and the obligation (kamino)
        for reserve in &obligation_reserves {
            refresh_kamino_reserve(
                svm,
                &super_authority,
                reserve,
                &kamino_config.market,
                scope_prices,
            )?;
        }

        refresh_kamino_obligation(
            svm,
            super_authority,
            &kamino_config.market,
            obligation,
            obligation_reserves,
        )?;

        let pull_ix = create_pull_kamino_lend_ix(
            &controller_pk,
            &integration_pk,
            &super_authority.pubkey(),
            &kamino_config,
            reserve_farm_collateral,
            reserve_vault_token_program,
            amount,
        );

        Ok(pull_ix)
    }

    fn assert_kamino_integration_at_init(
        integration: &Integration,
        kamino_config: &KaminoConfig,
        controller_pk: &Pubkey,
        rate_limit_slope: u64,
        rate_limit_max_outflow: u64,
        permit_liquidation: bool,
        clock: &Clock,
    ) {
        assert_eq!(integration.controller, *controller_pk);
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
                assert_eq!(config, *kamino_config)
            }
            _ => panic!("invalid config"),
        }

        let kamino_state = match integration.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        assert_eq!(kamino_state.last_liquidity_value, 0);
        assert_eq!(kamino_state.last_lp_amount, 0);
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Liquidity mint Token, Reward mint Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Liquidity mint T2022, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Liquidity mint T2022, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Liquidity mint Token, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint T2022, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), Some(0) ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint Token, Reward mint T2022 TransferFee 0 bps")]
    fn test_kamino_init_success(
        liquidity_mint_token_program: Pubkey,
        reward_mint_token_program: Pubkey,
        liquidity_mint_transfer_fee: Option<u16>,
        reward_mint_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &liquidity_mint_token_program,
            liquidity_mint_transfer_fee,
        )?;

        let reward_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &reward_mint_token_program,
            reward_mint_transfer_fee,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &liquidity_mint_token_program,
            &reward_mint,
            &reward_mint_token_program,
        );

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id,
            &controller_authority,
            &lending_market,
        );

        let kamino_config = KaminoConfig {
            market: lending_market,
            reserve: reserve_context.kamino_reserve_pk,
            reserve_liquidity_mint: liquidity_mint,
            obligation,
            obligation_id,
            padding: [0; 95],
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
            &liquidity_mint,
            obligation_id,
            &liquidity_mint_token_program,
        )
        .map_err(|e| {
            println!("error in setup_env_and_get_init_ix: {}", e);
            e
        })?;

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let clock = svm.get_sysvar::<Clock>();

        let integration = fetch_integration_account(&svm, &integration_pk)
            .expect("integration should exist")
            .unwrap();

        // assert integration was properly set
        assert_kamino_integration_at_init(
            &integration,
            &kamino_config,
            &controller_pk,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &clock,
        );

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
        let obligation_farm_collateral_pk =
            derive_obligation_farm_address(&reserve_context.reserve_farm_collateral, &obligation);

        let obligation_farm_collateral = svm.get_account(&obligation_farm_collateral_pk).unwrap();
        assert!(obligation_farm_collateral
            .owner
            .eq(&KAMINO_FARMS_PROGRAM_ID));
        assert!(obligation_farm_collateral.data.len() == 920);

        // assert obligation farm debt was created
        let obligation_farm_debt_pk =
            derive_obligation_farm_address(&reserve_context.reserve_farm_debt, &obligation);

        let obligation_farm_debt = svm.get_account(&obligation_farm_debt_pk).unwrap();
        assert!(obligation_farm_debt.owner.eq(&KAMINO_FARMS_PROGRAM_ID));
        assert!(obligation_farm_debt.data.len() == 920);

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Liquidity mint Token, Reward mint Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Liquidity mint T2022, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Liquidity mint T2022, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Liquidity mint Token, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint T2022, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), Some(0) ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint Token, Reward mint T2022 TransferFee 0 bps")]
    fn test_kamino_push_success(
        liquidity_mint_token_program: Pubkey,
        reward_mint_token_program: Pubkey,
        liquidity_mint_transfer_fee: Option<u16>,
        reward_mint_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &liquidity_mint_token_program,
            liquidity_mint_transfer_fee,
        )?;

        let reward_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &reward_mint_token_program,
            reward_mint_transfer_fee,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &liquidity_mint_token_program,
            &reward_mint,
            &reward_mint_token_program,
        );

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id,
            &controller_authority,
            &lending_market,
        );

        let kamino_config = KaminoConfig {
            market: lending_market,
            reserve: reserve_context.kamino_reserve_pk,
            reserve_liquidity_mint: liquidity_mint,
            obligation,
            obligation_id,
            padding: [0; 95],
        };

        let reserve_liquidity_destination = derive_reserve_liquidity_supply(
            &kamino_config.market,
            &kamino_config.reserve_liquidity_mint,
        );
        let reserve_collateral_destination = derive_reserve_collateral_supply(
            &kamino_config.market,
            &kamino_config.reserve_liquidity_mint,
        );

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (kamino_init_ix, integration_pk, reserve_keys) = setup_env_and_get_init_ix(
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
            &liquidity_mint,
            obligation_id,
            &liquidity_mint_token_program,
        )
        .unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let integration_before = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let balance_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let reserve_liquidity_destination_balance_before =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_before =
            get_token_balance_or_zero(&svm, &reserve_collateral_destination);

        let (liquidity_value_before, lp_amount_before) =
            get_liquidity_and_lp_amount(&svm, &kamino_config.reserve, &kamino_config.obligation)?;

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
            &reserve_context.reserve_farm_collateral,
            &liquidity_mint_token_program,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;
        println!("tx_result logs: {}", tx_result.pretty_logs());

        let reserve_liquidity_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_collateral_destination);

        let liquidity_amount_kamino_vault_delta = reserve_liquidity_destination_balance_after
            - reserve_liquidity_destination_balance_before;
        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        // actual amount deposited in kamino
        let balance_delta = balance_before - balance_after;

        let integration_after = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();

        let (liquidity_value_after, lp_amount_after) =
            get_liquidity_and_lp_amount(&svm, &kamino_config.reserve, &kamino_config.obligation)?;

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

        // Assert Reserve vault was debited exact amount
        assert_eq!(
            balance_after,
            balance_before - liquidity_amount_kamino_vault_delta
        );

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
        assert_eq!(state_after.last_liquidity_value, liquidity_value_after);
        assert_eq!(state_after.last_lp_amount, lp_amount_after);
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
            delta: 1_000_000_000_000,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            reserve_sync_expected_event
        );

        let integration_credit_expected_event =
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pk),
                mint: kamino_config.reserve_liquidity_mint,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Deposit,
                delta: liquidity_value_delta,
            });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            integration_credit_expected_event
        );

        let reserve_debit_expected_event =
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: None,
                mint: kamino_config.reserve_liquidity_mint,
                reserve: Some(reserve_keys.pubkey),
                direction: AccountingDirection::Debit,
                action: AccountingAction::Deposit,
                delta: balance_delta,
            });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            reserve_debit_expected_event
        );

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Liquidity mint Token, Reward mint Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Liquidity mint T2022, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Liquidity mint T2022, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Liquidity mint Token, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint T2022, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), Some(0) ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint Token, Reward mint T2022 TransferFee 0 bps")]
    fn test_kamino_pull_success(
        liquidity_mint_token_program: Pubkey,
        reward_mint_token_program: Pubkey,
        liquidity_mint_transfer_fee: Option<u16>,
        reward_mint_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &liquidity_mint_token_program,
            liquidity_mint_transfer_fee,
        )?;

        let reward_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &reward_mint_token_program,
            reward_mint_transfer_fee,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &liquidity_mint_token_program,
            &reward_mint,
            &reward_mint_token_program,
        );

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id,
            &controller_authority,
            &lending_market,
        );

        let kamino_config = KaminoConfig {
            market: lending_market,
            reserve: reserve_context.kamino_reserve_pk,
            reserve_liquidity_mint: liquidity_mint,
            obligation,
            obligation_id,
            padding: [0; 95],
        };

        let reserve_liquidity_destination = derive_reserve_liquidity_supply(
            &kamino_config.market,
            &kamino_config.reserve_liquidity_mint,
        );
        let reserve_collateral_destination = derive_reserve_collateral_supply(
            &kamino_config.market,
            &kamino_config.reserve_liquidity_mint,
        );

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (kamino_init_ix, integration_pk, reserve_keys) = setup_env_and_get_init_ix(
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
            &liquidity_mint,
            obligation_id,
            &liquidity_mint_token_program,
        )
        .unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            100_000_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &liquidity_mint_token_program,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();

        svm.expire_blockhash();

        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();
        let integration_before = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let balance_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let reserve_liquidity_destination_balance_before =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_before =
            get_token_balance_or_zero(&svm, &reserve_collateral_destination);

        let (liquidity_value_before, lp_amount_before) =
            get_liquidity_and_lp_amount(&svm, &kamino_config.reserve, &kamino_config.obligation)?;

        let pull_ix = get_pull_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            100_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &liquidity_mint_token_program,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, pull_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let (liquidity_value_after, lp_amount_after) =
            get_liquidity_and_lp_amount(&svm, &kamino_config.reserve, &kamino_config.obligation)?;

        let reserve_liquidity_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_collateral_destination);
        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        // actual withdrawal amount
        let balance_delta = balance_after - balance_before;

        let liquidity_amount_kamino_vault_delta = reserve_liquidity_destination_balance_before
            - reserve_liquidity_destination_balance_after;

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
        assert_eq!(
            balance_after,
            balance_before + liquidity_amount_kamino_vault_delta
        );

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
        assert_eq!(state_after.last_liquidity_value, liquidity_value_after);
        assert_eq!(state_after.last_lp_amount, lp_amount_after);
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

        let integration_debit_expected_event =
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
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

        let reserve_credit_expected_event =
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
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

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Liquidity mint Token, Reward mint Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Liquidity mint T2022, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Liquidity mint T2022, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Liquidity mint Token, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint T2022, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(0), Some(0) ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint T2022 TransferFee 0 bps")]
    #[test_case( spl_token_2022::ID, spl_token::ID, Some(0), None ; "Liquidity mint T2022 TransferFee 0 bps, Reward mint Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, Some(0) ; "Liquidity mint Token, Reward mint T2022 TransferFee 0 bps")]
    fn test_kamino_sync_success(
        liquidity_mint_token_program: Pubkey,
        _reward_mint_token_program: Pubkey,
        liquidity_mint_transfer_fee: Option<u16>,
        _reward_mint_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &liquidity_mint_token_program,
            liquidity_mint_transfer_fee,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &liquidity_mint_token_program,
            &liquidity_mint,
            &liquidity_mint_token_program,
        );

        let obligation_id = 0;
        let obligation = derive_vanilla_obligation_address(
            obligation_id,
            &controller_authority,
            &lending_market,
        );

        let kamino_config = KaminoConfig {
            market: lending_market,
            reserve: reserve_context.kamino_reserve_pk,
            reserve_liquidity_mint: liquidity_mint,
            obligation,
            obligation_id,
            padding: [0; 95],
        };

        // in order to trigger all accounting events in sync, we set the reward mint
        // to equal the reserve mint
        let reward_mint = kamino_config.reserve_liquidity_mint;

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (kamino_init_ix, integration_pk, reserve_keys) = setup_env_and_get_init_ix(
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
            &liquidity_mint,
            obligation_id,
            &liquidity_mint_token_program,
        )
        .unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // Deposit some amount into kamino
        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            100_000_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &liquidity_mint_token_program,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        let rewards_ata = get_associated_token_address_with_program_id(
            &controller_authority,
            &reward_mint,
            &liquidity_mint_token_program,
        );

        // increase the amount in the integration reserve in order to
        // trigger the first event in reserve.sync_balance
        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)?.unwrap();

        edit_ata_amount(
            &mut svm,
            &controller_authority,
            &kamino_config.reserve_liquidity_mint,
            1_100_000_000_000,
        )?;

        // increase the liquidity amount available in the kamino reserve
        // in order to trigger liquidity value change event

        let (liquidity_value_before, lp_amount_before) =
            get_liquidity_and_lp_amount(&svm, &kamino_config.reserve, &kamino_config.obligation)?;

        let kamino_reserve = fetch_kamino_reserve(&svm, &kamino_config.reserve)?;
        let new_kamino_reserve_liq_available_amount =
            kamino_reserve.liquidity.available_amount + 100_000_000_000;
        set_kamino_reserve_liquidity_available_amount(
            &mut svm,
            &kamino_config.reserve,
            new_kamino_reserve_liq_available_amount,
        )?;

        let (liquidity_value_after, lp_amount_after) =
            get_liquidity_and_lp_amount(&svm, &kamino_config.reserve, &kamino_config.obligation)?;

        let integration_before = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reward_ata_balance_before = get_token_balance_or_zero(&svm, &rewards_ata);

        let obligation_collateral_farm =
            derive_obligation_farm_address(&reserve_context.reserve_farm_collateral, &obligation);

        // increase unclaimed rewards of obligation farm
        let rewards_unclaimed = 100_000_000;
        set_obligation_farm_rewards_issued_unclaimed(
            &mut svm,
            &obligation_collateral_farm,
            &reward_mint,
            &liquidity_mint_token_program,
            rewards_unclaimed,
        )?;

        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk,
            &integration_pk,
            &super_authority.pubkey(),
            &kamino_config,
            &reward_mint,
            &farms_context.global_config,
            &reserve_context.reserve_farm_collateral,
            &rewards_ata,
            // since we are setting farm_collateral.scope_oracle_price_id = u64::MAX,
            // no scope_price is required, in order to pass None we need to pass
            // KFARMS program ID.
            &KAMINO_FARMS_PROGRAM_ID,
            &liquidity_mint_token_program,
            &liquidity_mint_token_program,
        );
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, sync_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let reward_ata_balance_after = get_token_balance_or_zero(&svm, &rewards_ata);

        let reward_ata_balance_delta =
            reward_ata_balance_after.saturating_sub(reward_ata_balance_before);

        let integration_after = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)?.unwrap();

        // assert lp amount didnt change
        assert_eq!(lp_amount_after, lp_amount_before);

        // assert the reward ata delta is equal to the rewards unclaimed in obligation farm
        assert_eq!(rewards_unclaimed, reward_ata_balance_delta);

        // Assert emitted events

        let liq_value_delta = reserve_after
            .last_balance
            .abs_diff(reserve_before.last_balance);
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

        // assert sync event for credit (inflow) integration
        // emitted since harvest mint matches the integration reserve mint
        let expected_credit_integration_event =
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pk),
                reserve: None,
                direction: AccountingDirection::Credit,
                mint: kamino_config.reserve_liquidity_mint,
                action: AccountingAction::Sync,
                delta: rewards_unclaimed,
            });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            expected_credit_integration_event
        );

        // assert accounting event for debit (outflow) integration
        // emitted since harvest mint matches the integration reserve mint
        let expected_debit_integration_event =
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pk),
                reserve: None,
                direction: AccountingDirection::Debit,
                mint: kamino_config.reserve_liquidity_mint,
                action: AccountingAction::Withdrawal,
                delta: rewards_unclaimed,
            });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            expected_debit_integration_event
        );

        // assert accounting event for credit (inflow) reserve
        // emitted since harvest mint matches the integration reserve mint
        let expected_credit_reserve_event =
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: None,
                reserve: Some(reserve_keys.pubkey),
                direction: AccountingDirection::Credit,
                mint: kamino_config.reserve_liquidity_mint,
                action: AccountingAction::Withdrawal,
                delta: reward_ata_balance_delta,
            });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            expected_credit_reserve_event
        );

        // assert liquidity_value change event was emitted
        let liq_value_delta = liquidity_value_after.abs_diff(liquidity_value_before);
        let expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pk),
            reserve: None,
            mint: kamino_config.reserve_liquidity_mint,
            action: AccountingAction::Sync,
            delta: liq_value_delta,
            direction: AccountingDirection::Credit,
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
        assert_eq!(state_after.last_lp_amount, state_before.last_lp_amount);

        Ok(())
    }

    #[test]
    fn test_kamino_multiple_reserves_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context,
        } = setup_kamino_state(
            &mut svm,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
        );

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
            padding: [0; 95],
        };

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;
        let permit_liquidation = true;

        let (kamino_init_ix, _integration_pk, _reserve_keys) = setup_env_and_get_init_ix(
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
            obligation_id,
            &spl_token::ID,
        )
        .unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // create two additional mints for two reserves
        let mint_1 = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;
        let mint_2 = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        // create two new controller reserves
        let _reserve_1 = create_and_fund_controller_reserves(
            &mut svm,
            &controller_pk,
            &super_authority,
            &controller_authority,
            &mint_1,
            &spl_token::ID,
        )?;
        let _reserve_2 = create_and_fund_controller_reserves(
            &mut svm,
            &controller_pk,
            &super_authority,
            &controller_authority,
            &mint_2,
            &spl_token::ID,
        )?;

        // create two new kamino reserves

        let contexts = setup_additional_reserves(
            &mut svm,
            &farms_context.global_config,
            &lending_market,
            (&USDC_TOKEN_MINT_PUBKEY, &spl_token::ID),
            vec![(&mint_1, &spl_token::ID), (&mint_2, &spl_token::ID)],
        );
        let [context_1, context_2] = contexts.as_slice() else {
            panic!("error");
        };

        let kamino_config_1 = KaminoConfig {
            market: lending_market,
            reserve: context_1.kamino_reserve_pk,
            reserve_liquidity_mint: mint_1,
            obligation,
            obligation_id,
            padding: [0; 95],
        };

        let kamino_config_2 = KaminoConfig {
            market: lending_market,
            reserve: context_2.kamino_reserve_pk,
            reserve_liquidity_mint: mint_2,
            obligation,
            obligation_id,
            padding: [0; 95],
        };

        let (kamino_init_ix_1, kamino_integration_pk_1) =
            create_initialize_kamino_lend_integration_ix(
                &controller_pk,
                &super_authority.pubkey(),
                &super_authority.pubkey(),
                &description,
                status,
                rate_limit_slope,
                rate_limit_max_outflow,
                permit_liquidation,
                &IntegrationConfig::Kamino(kamino_config_1.clone()),
                &context_1.reserve_farm_collateral,
                &context_1.reserve_farm_debt,
                obligation_id,
                &KAMINO_LEND_PROGRAM_ID,
            );

        let (kamino_init_ix_2, kamino_integration_pk_2) =
            create_initialize_kamino_lend_integration_ix(
                &controller_pk,
                &super_authority.pubkey(),
                &super_authority.pubkey(),
                &description,
                status,
                rate_limit_slope,
                rate_limit_max_outflow,
                permit_liquidation,
                &IntegrationConfig::Kamino(kamino_config_2.clone()),
                &context_2.reserve_farm_collateral,
                &context_2.reserve_farm_debt,
                obligation_id,
                &KAMINO_LEND_PROGRAM_ID,
            );

        // initialize the two new integrations (same obligation under the hood)
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix_1, kamino_init_ix_2],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let clock = svm.get_sysvar::<Clock>();

        let integration_1 = fetch_integration_account(&svm, &kamino_integration_pk_1)
            .expect("integration should exist")
            .unwrap();

        let integration_2 = fetch_integration_account(&svm, &kamino_integration_pk_2)
            .expect("integration should exist")
            .unwrap();

        assert_kamino_integration_at_init(
            &integration_1,
            &kamino_config_1,
            &controller_pk,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &clock,
        );

        assert_kamino_integration_at_init(
            &integration_2,
            &kamino_config_2,
            &controller_pk,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            &clock,
        );

        // push into the two integration
        let deposited_amount_1 = 100_000_000;
        let push_ix_1 = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &kamino_integration_pk_1,
            &obligation,
            &kamino_config_1,
            deposited_amount_1,
            &Pubkey::default(),
            &context_1.reserve_farm_collateral,
            &SPL_TOKEN_PROGRAM_ID,
        )?;

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix_1],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        // assert the obligation collateral slot was filled with a new ObligationCollateral
        // for the new reserve
        let obligation_after = fetch_kamino_obligation(&svm, &obligation)?;

        assert!(obligation_after
            .get_obligation_collateral_for_reserve(&context_1.kamino_reserve_pk)
            .is_some());

        svm.expire_blockhash();

        // push + pull + sync with kamino reserve 2
        let deposited_amount_2 = 200_000_000;
        let push_ix_2 = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &kamino_integration_pk_2,
            &obligation,
            &kamino_config_2,
            deposited_amount_2,
            &Pubkey::default(),
            &context_2.reserve_farm_collateral,
            &SPL_TOKEN_PROGRAM_ID,
        )?;

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix_2],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        // assert the obligation collateral slot was filled with a new ObligationCollateral
        // for the new kamino reserve
        let obligation_after_push = fetch_kamino_obligation(&svm, &obligation)?;

        assert!(obligation_after_push
            .get_obligation_collateral_for_reserve(&context_2.kamino_reserve_pk)
            .is_some());

        svm.expire_blockhash();

        // pull
        let total_collateral_amount_before = obligation_after_push
            .get_obligation_collateral_for_reserve(&context_2.kamino_reserve_pk)
            .unwrap()
            .deposited_amount;

        let collateral_amount = 100_000;
        let pull_ix = get_pull_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &kamino_integration_pk_2,
            &obligation,
            &kamino_config_2,
            collateral_amount,
            &Pubkey::default(),
            &context_2.reserve_farm_collateral,
            &spl_token::ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, pull_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let obligation_after_pull = fetch_kamino_obligation(&svm, &obligation)?;
        let total_collateral_amount_after = obligation_after_pull
            .get_obligation_collateral_for_reserve(&context_2.kamino_reserve_pk)
            .unwrap()
            .deposited_amount;

        // assert the amount held in ObligationCollateral decreases
        assert_eq!(
            total_collateral_amount_after + collateral_amount,
            total_collateral_amount_before
        );

        // sync ix
        // increase kamino reserve liquidity to increase the integration liquidity value
        let kamino_reserve = fetch_kamino_reserve(&svm, &kamino_config_2.reserve)?;
        let new_kamino_reserve_liq_available_amount =
            kamino_reserve.liquidity.available_amount + 100_000_000_000;
        set_kamino_reserve_liquidity_available_amount(
            &mut svm,
            &kamino_config_2.reserve,
            new_kamino_reserve_liq_available_amount,
        )?;

        let integration_before_sync = fetch_integration_account(&svm, &kamino_integration_pk_2)
            .unwrap()
            .unwrap();

        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk,
            &kamino_integration_pk_2,
            &super_authority.pubkey(),
            &kamino_config_2,
            &USDC_TOKEN_MINT_PUBKEY,
            &farms_context.global_config,
            &context_2.reserve_farm_collateral,
            // rewards_ata set to default to skip harvesting rewards
            &Pubkey::default(),
            // since we are setting farm_collateral.scope_oracle_price_id = u64::MAX,
            // no scope_price is required, in order to pass None we need to pass
            // KFARMS program ID.
            &KAMINO_FARMS_PROGRAM_ID,
            &SPL_TOKEN_PROGRAM_ID,
            &SPL_TOKEN_PROGRAM_ID,
        );
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, sync_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let integration_after_sync = fetch_integration_account(&svm, &kamino_integration_pk_2)
            .unwrap()
            .unwrap();

        // assert integration state changed
        let state_before = match integration_before_sync.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after_sync.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };

        assert!(state_before.last_liquidity_value < state_after.last_liquidity_value);

        //assert lp amount did not change
        assert_eq!(state_after.last_lp_amount, state_before.last_lp_amount);

        Ok(())
    }

    #[test_case(true, false, false, false, false, false, false, false, false, false, false; "can_manage_permissions fails")]
    #[test_case(false, true, false, false, false, false, false, false, false, false, false; "can_invoke_external_transfer fails")]
    #[test_case(false, false, true, false, false, false, false, false, false, false, false; "can_execute_swap fails")]
    #[test_case(false, false, false, true, false, false, false, false, false, false, true; "can_reallocate passes")]
    #[test_case(false, false, false, false, true, false, false, false, false, false, false; "can_freeze_controller fails")]
    #[test_case(false, false, false, false, false, true, false, false, false, false, false; "can_unfreeze_controller fails")]
    #[test_case(false, false, false, false, false, false, true, false, false, false, false; "can_manage_reserves_and_integrations fails")]
    #[test_case(false, false, false, false, false, false, false, true, false, false, false; "can_suspend_permissions fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, false, false; "can_liquidate w/o permit_liquidation fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, true, false; "can_liquidate w/ permit_liquidation fails")]
    fn test_kamino_push_permissions(
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_execute_swap: bool,
        can_reallocate: bool,
        can_freeze_controller: bool,
        can_unfreeze_controller: bool,
        can_manage_reserves_and_integrations: bool,
        can_suspend_permissions: bool,
        can_liquidate: bool,
        permit_liquidation: bool,
        result_ok: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _,
        } = setup_kamino_state(
            &mut svm,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
        );

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
            padding: [0; 95],
        };

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;

        let (kamino_init_ix, integration_pk, _reserve_keys) = setup_env_and_get_init_ix(
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
            obligation_id,
            &spl_token::ID,
        )
        .unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        let push_authority = Keypair::new();
        airdrop_lamports(&mut svm, &push_authority.pubkey(), 1_000_000_000)?;
        // Update the authority to have permissions
        manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,         // payer
            &super_authority,         // calling authority
            &push_authority.pubkey(), // subject authority
            PermissionStatus::Active,
            can_execute_swap,                     // can_execute_swap,
            can_manage_permissions,               // can_manage_permissions,
            can_invoke_external_transfer,         // can_invoke_external_transfer,
            can_reallocate,                       // can_reallocate,
            can_freeze_controller,                // can_freeze,
            can_unfreeze_controller,              // can_unfreeze,
            can_manage_reserves_and_integrations, // can_manage_reserves_and_integrations
            can_suspend_permissions,              // can_suspend_permissions
            can_liquidate,                        // can_liquidate
        )?;

        let deposited_amount = 100_000_000;
        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &push_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            deposited_amount,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &SPL_TOKEN_PROGRAM_ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&push_authority.pubkey()),
            &[&push_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);

        // Assert the expected result given the enabled privilege
        match result_ok {
            true => assert!(tx_result.is_ok()),
            false => assert_eq!(
                tx_result.err().unwrap().err,
                TransactionError::InstructionError(1, InstructionError::IncorrectAuthority)
            ),
        }
        Ok(())
    }

    #[test_case(true, false, false, false, false, false, false, false, false, false, false; "can_manage_permissions fails")]
    #[test_case(false, true, false, false, false, false, false, false, false, false, false; "can_invoke_external_transfer fails")]
    #[test_case(false, false, true, false, false, false, false, false, false, false, false; "can_execute_swap fails")]
    #[test_case(false, false, false, true, false, false, false, false, false, false, true; "can_reallocate passes")]
    #[test_case(false, false, false, false, true, false, false, false, false, false, false; "can_freeze_controller fails")]
    #[test_case(false, false, false, false, false, true, false, false, false, false, false; "can_unfreeze_controller fails")]
    #[test_case(false, false, false, false, false, false, true, false, false, false, false; "can_manage_reserves_and_integrations fails")]
    #[test_case(false, false, false, false, false, false, false, true, false, false, false; "can_suspend_permissions fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, false, false; "can_liquidate w/o permit_liquidation fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, true, true; "can_liquidate w/ permit_liquidation passes")]
    fn test_kamino_pull_permissions(
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_execute_swap: bool,
        can_reallocate: bool,
        can_freeze_controller: bool,
        can_unfreeze_controller: bool,
        can_manage_reserves_and_integrations: bool,
        can_suspend_permissions: bool,
        can_liquidate: bool,
        permit_liquidation: bool,
        result_ok: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _,
        } = setup_kamino_state(
            &mut svm,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
        );

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
            padding: [0; 95],
        };

        let description = "test";
        let status = IntegrationStatus::Active;
        let rate_limit_slope = 100_000_000_000;
        let rate_limit_max_outflow = 100_000_000_000;

        let (kamino_init_ix, integration_pk, _reserve_keys) = setup_env_and_get_init_ix(
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
            obligation_id,
            &spl_token::ID,
        )
        .unwrap();

        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, kamino_init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            100_000_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &SPL_TOKEN_PROGRAM_ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();

        let pull_authority = Keypair::new();
        airdrop_lamports(&mut svm, &pull_authority.pubkey(), 1_000_000_000)?;
        // Update the authority to have permissions
        manage_permission(
            &mut svm,
            &controller_pk,
            &super_authority,         // payer
            &super_authority,         // calling authority
            &pull_authority.pubkey(), // subject authority
            PermissionStatus::Active,
            can_execute_swap,                     // can_execute_swap,
            can_manage_permissions,               // can_manage_permissions,
            can_invoke_external_transfer,         // can_invoke_external_transfer,
            can_reallocate,                       // can_reallocate,
            can_freeze_controller,                // can_freeze,
            can_unfreeze_controller,              // can_unfreeze,
            can_manage_reserves_and_integrations, // can_manage_reserves_and_integrations
            can_suspend_permissions,              // can_suspend_permissions
            can_liquidate,                        // can_liquidate
        )?;

        let pull_ix = get_pull_ix(
            &mut svm,
            &controller_pk,
            &pull_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            100_000,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &spl_token::ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, pull_ix],
            Some(&pull_authority.pubkey()),
            &[&pull_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);

        // Assert the expected result given the enabled privilege
        match result_ok {
            true => assert!(tx_result.is_ok()),
            false => assert_eq!(
                tx_result.err().unwrap().err,
                TransactionError::InstructionError(1, InstructionError::IncorrectAuthority)
            ),
        }
        Ok(())
    }
}
