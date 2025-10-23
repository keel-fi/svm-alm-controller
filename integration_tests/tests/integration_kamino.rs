mod helpers;
mod subs;

mod tests {
    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{
            assert::assert_custom_error,
            constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, USDC_TOKEN_MINT_PUBKEY},
            kamino::state::klend::{KaminoReserve, LastUpdate, Obligation},
            setup_test_controller,
            spl::SPL_TOKEN_PROGRAM_ID,
            TestContext,
        },
        subs::{
            airdrop_lamports, derive_controller_authority_pda, edit_ata_amount,
            fetch_integration_account, fetch_kamino_obligation, fetch_reserve_account,
            get_token_balance_or_zero, initialize_ata, initialize_mint, initialize_reserve,
            kamino_reserve_accrue_interest, manage_permission, refresh_kamino_obligation,
            refresh_kamino_reserve, set_obligation_farm_rewards_issued_unclaimed,
            setup_additional_reserves, setup_kamino_state, transfer_tokens, KaminoTestContext,
            ReserveKeys,
        },
        test_invalid_accounts,
    };
    use borsh::BorshDeserialize;
    use litesvm::LiteSVM;
    use solana_sdk::{
        account::Account,
        clock::Clock,
        compute_budget::ComputeBudgetInstruction,
        instruction::{Instruction, InstructionError},
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_program,
        transaction::{Transaction, TransactionError},
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use svm_alm_controller::error::SvmAlmControllerErrors;
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
        sync_integration::{create_sync_kamino_lend_ix, HarvestRewardAccounts},
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

    fn create_account_clone_w_new_pk(svm: &mut LiteSVM, acc_pk: &Pubkey) -> Pubkey {
        let acc_data = svm.get_account(&acc_pk).expect("failed to fetch account");
        let new_acc_pk = Pubkey::new_unique();
        svm.set_account(new_acc_pk, acc_data)
            .expect("failed to set account");
        new_acc_pk
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

        let obligation_reserves: Vec<&Pubkey> =
            if let Ok(obligation_state) = Obligation::try_from(&obligation_acc.data) {
                obligation_state
                    .deposits
                    .iter()
                    .filter_map(|deposit| {
                        if deposit.deposit_reserve != Pubkey::default() {
                            return Some(&deposit.deposit_reserve);
                        }
                        None
                    })
                    .collect()
            } else {
                // always refresh the current reserve in case we are re initializing an obligation
                vec![&kamino_config.reserve]
            };

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

        if obligation_acc.owner == KAMINO_LEND_PROGRAM_ID {
            refresh_kamino_obligation(
                svm,
                super_authority,
                &kamino_config.market,
                obligation,
                obligation_reserves,
            )?;
        }

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
        assert_eq!(kamino_state.balance, 0);
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
            10_000,
            true,
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
            // 1 collateral : 2 liquidity
            5_000,
            true,
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

        let push_amount = 100_000_000;
        let lp_push_amount = 50_000_000; // 1:2 ratio set in `setup_kamino_state`
        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            push_amount,
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

        let reserve_liquidity_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_collateral_destination);

        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        let integration_after = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();

        // Assert Integration rate limits adjusted
        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_before.rate_limit_outflow_amount_available - push_amount
        );

        // Assert Reserve rate limits adjusted
        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_before.rate_limit_outflow_amount_available - push_amount
        );

        // Assert Reserve vault was debited exact amount
        assert_eq!(balance_after, balance_before - push_amount);

        // Assert kamino's token account received the tokens
        assert_eq!(
            reserve_liquidity_destination_balance_after,
            reserve_liquidity_destination_balance_before + push_amount
        );

        // Assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        assert_eq!(state_after.balance, push_amount);
        assert_eq!(state_after.balance, state_before.balance + push_amount,);

        // Assert LP Vault balance increased
        assert_eq!(
            reserve_collateral_destination_balance_after,
            reserve_collateral_destination_balance_before + lp_push_amount
        );

        let vault_delta = reserve_collateral_destination_balance_after
            .saturating_sub(reserve_collateral_destination_balance_before);

        assert_eq!(vault_delta, lp_push_amount);

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
                delta: push_amount,
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
                delta: push_amount,
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
            // 1 collateral : 2 liquidity
            5_000,
            true,
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

        let push_amount = 100_000_000;
        let _lp_push_amount = 50_000_000; // 1:2 ratio set in `setup_kamino_state`
        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            push_amount,
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

        let pull_amount = 100_000;
        let lp_pull_amount = 50_000; // 1:2 ratio set in `setup_kamino_state`
        let pull_ix = get_pull_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            pull_amount,
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

        let reserve_liquidity_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);
        let reserve_collateral_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_collateral_destination);
        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let integration_after = fetch_integration_account(&svm, &integration_pk)
            .unwrap()
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .unwrap()
            .unwrap();

        // Assert integration rate limits adjusted
        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_before.rate_limit_outflow_amount_available + pull_amount
        );

        // Assert Reserve rate limits adjusted
        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_before.rate_limit_outflow_amount_available + pull_amount
        );

        // Assert Reserve vault was credited exact amount
        assert_eq!(balance_after, balance_before + pull_amount);

        // Assert kamino's token account balance decreased
        assert_eq!(
            reserve_liquidity_destination_balance_after,
            reserve_liquidity_destination_balance_before - pull_amount
        );

        // Assert LP Vault balance decreased
        let vault_delta = reserve_collateral_destination_balance_before
            .saturating_sub(reserve_collateral_destination_balance_after);

        assert_eq!(vault_delta, lp_pull_amount);

        // assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::Kamino(kamino_state) => kamino_state,
            _ => panic!("invalid state"),
        };
        assert_eq!(state_after.balance, push_amount - pull_amount);
        assert_eq!(state_after.balance, state_before.balance - pull_amount,);

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
                delta: pull_amount,
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
                delta: pull_amount,
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
            10_000,
            true,
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

        let deposit_amount = 100_000_000;
        // Deposit some amount into kamino
        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            deposit_amount,
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

        // Accrue 1% interest
        let interest_bps = 100;
        kamino_reserve_accrue_interest(&mut svm, &kamino_config.reserve, interest_bps)?;
        let interest_on_deposit = deposit_amount * interest_bps / 10_000;

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

        // Refresh the kamino reserve to ensure it's not stale before sync
        refresh_kamino_reserve(
            &mut svm,
            &super_authority,
            &kamino_config.reserve,
            &kamino_config.market,
            &KAMINO_FARMS_PROGRAM_ID,
        )?;

        let harvest_acounts = HarvestRewardAccounts {
            rewards_mint: &reward_mint,
            global_config: &farms_context.global_config,
            reserve_farm_collateral: &reserve_context.reserve_farm_collateral,
            scope_prices: &KAMINO_FARMS_PROGRAM_ID,
            rewards_token_program: &liquidity_mint_token_program,
        };

        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk,
            &integration_pk,
            &super_authority.pubkey(),
            &kamino_config,
            &liquidity_mint_token_program,
            Some(harvest_acounts),
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
        let expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pk),
            reserve: None,
            mint: kamino_config.reserve_liquidity_mint,
            action: AccountingAction::Sync,
            delta: interest_on_deposit,
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
            state_before.balance + interest_on_deposit,
            state_after.balance
        );

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
            10_000,
            true,
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
        // Accrue 1% interest
        kamino_reserve_accrue_interest(&mut svm, &kamino_config_2.reserve, 100)?;

        let integration_before_sync = fetch_integration_account(&svm, &kamino_integration_pk_2)
            .unwrap()
            .unwrap();

        // Refresh the kamino reserve to ensure it's not stale before sync
        refresh_kamino_reserve(
            &mut svm,
            &super_authority,
            &kamino_config_2.reserve,
            &kamino_config_2.market,
            &KAMINO_FARMS_PROGRAM_ID,
        )?;

        let harvest_acounts = HarvestRewardAccounts {
            rewards_mint: &USDC_TOKEN_MINT_PUBKEY,
            global_config: &farms_context.global_config,
            reserve_farm_collateral: &context_2.reserve_farm_collateral,
            scope_prices: &KAMINO_FARMS_PROGRAM_ID,
            rewards_token_program: &SPL_TOKEN_PROGRAM_ID,
        };

        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk,
            &kamino_integration_pk_2,
            &super_authority.pubkey(),
            &kamino_config_2,
            &SPL_TOKEN_PROGRAM_ID,
            Some(harvest_acounts),
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

        assert!(state_before.balance < state_after.balance);

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
            10_000,
            true,
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
            10_000,
            true,
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

    #[test]
    fn test_kamino_obligation_closure() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint_token_program = spl_token::ID;
        let liquidity_mint_transfer_fee = None;
        let reward_mint_token_program = spl_token::ID;
        let reward_mint_transfer_fee = None;

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
            10_000,
            true,
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

        // withdraw all liquidity deposited, which closes the Obligation
        let pull_ix = get_pull_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            u64::MAX,
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
        svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        svm.expire_blockhash();

        // assert obligation was closed
        let obligation_acc = svm
            .get_account(&obligation)
            .expect("Failed to fetch obligation");
        assert_eq!(obligation_acc.owner, system_program::ID);

        // deposit again in same integration
        let reserve_liquidity_destination = derive_reserve_liquidity_supply(
            &kamino_config.market,
            &kamino_config.reserve_liquidity_mint,
        );

        let balance_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let reserve_liquidity_destination_balance_before =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);

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
        let _tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        // assert obligation was initialized
        let obligation_acc = svm
            .get_account(&obligation)
            .expect("Failed to fetch obligation");
        assert_eq!(obligation_acc.owner, KAMINO_LEND_PROGRAM_ID);

        let reserve_liquidity_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);

        let liquidity_amount_kamino_vault_delta = reserve_liquidity_destination_balance_after
            - reserve_liquidity_destination_balance_before;

        let balance_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let balance_delta = balance_before - balance_after;

        let reserve_liquidity_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);

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

        Ok(())
    }

    #[test]
    fn test_kamino_reserve_without_farms_success() -> Result<(), Box<dyn std::error::Error>> {
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
            &spl_token::ID,
            None,
        )?;

        let reward_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &spl_token::ID,
            &reward_mint,
            &spl_token::ID,
            10_000,
            false,
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
            &liquidity_mint,
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
            &spl_token::ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let _tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

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
            &spl_token::ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, pull_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let _tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let reserve_liquidity_destination_balance_after =
            get_token_balance_or_zero(&svm, &reserve_liquidity_destination);

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

        // Refresh the kamino reserve to ensure it's not stale before sync
        refresh_kamino_reserve(
            &mut svm,
            &super_authority,
            &kamino_config.reserve,
            &kamino_config.market,
            &KAMINO_FARMS_PROGRAM_ID,
        )?;

        // Initialize rewards ATA for harvesting
        let _rewards_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &controller_authority,
            &reward_mint,
        )?;

        let harvest_accounts = HarvestRewardAccounts {
            rewards_mint: &reward_mint,
            global_config: &farms_context.global_config,
            reserve_farm_collateral: &reserve_context.reserve_farm_collateral,
            scope_prices: &KAMINO_FARMS_PROGRAM_ID,
            rewards_token_program: &spl_token::ID,
        };

        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk,
            &integration_pk,
            &super_authority.pubkey(),
            &kamino_config,
            &spl_token::ID,
            None,
        );
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, sync_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let _tx_result = svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        Ok(())
    }

    #[test]
    fn test_kamino_init_inner_ctx_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>>
    {
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
            &spl_token::ID,
            None,
        )?;

        let reward_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context: _,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &spl_token::ID,
            &reward_mint,
            &spl_token::ID,
            10_000,
            true,
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
        let (mut kamino_init_ix, _integration_pk, _) = setup_env_and_get_init_ix(
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
            &liquidity_mint,
            obligation_id,
            &spl_token::ID,
        )
        .map_err(|e| {
            println!("error in setup_env_and_get_init_ix: {}", e);
            e
        })?;

        // Checks for inner_ctx accounts:
        // (index 8) obligation
        //      pubkey matches PDA
        // (index 9) reserve_liquidity_mint
        //      owned by spl token or token2022
        // (index 10) user_metadata
        //      pubkey matches PDA
        // (index 12) obligation_farm_collateral
        //      owned by KFARMS or system_program
        // (index 13) kamino_reserve
        //      owned by KLEND
        // (index 14) reserve_farm_collateral
        //      owned by KLEND OR pubkey == pubkey::default
        // (index 15) market_authority
        //      pubkey matches PDA
        // (index 16) market
        //      owned by KLEND
        // (index 17) kamino_program
        //      pubkey == KLEND
        // (index 18) kamino_farms_program
        //      pubkey == KFARMS
        // (index 19) system_program
        //      pubkey == system_program
        // (index 20) rent
        //      pubkey == sysvars rent

        // change obligation pubkey
        let obligation_account_pk = kamino_init_ix.accounts[8].pubkey;
        kamino_init_ix.accounts[8].pubkey = Pubkey::new_unique();
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[kamino_init_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidPda);
        kamino_init_ix.accounts[8].pubkey = obligation_account_pk;

        // change user_metadata pubkey
        let user_metadata_pk = kamino_init_ix.accounts[10].pubkey;
        kamino_init_ix.accounts[10].pubkey = Pubkey::new_unique();
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[kamino_init_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidPda);
        kamino_init_ix.accounts[10].pubkey = user_metadata_pk;

        // initialize obligation_farm_collateral with a different owner
        let obligation_farm_collateral_pk = kamino_init_ix.accounts[12].pubkey;
        svm.set_account(
            obligation_farm_collateral_pk,
            Account {
                lamports: u64::MAX,
                data: vec![1, 1, 1],
                owner: Pubkey::new_unique(),
                executable: false,
                rent_epoch: 0,
            },
        )?;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[kamino_init_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountOwner)
        );
        svm.set_account(obligation_farm_collateral_pk, Account::default())?;

        // modify market_authority pubkey
        let market_authority_pk = kamino_init_ix.accounts[15].pubkey;
        kamino_init_ix.accounts[15].pubkey = Pubkey::new_unique();
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[kamino_init_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidPda);
        kamino_init_ix.accounts[15].pubkey = market_authority_pk;

        svm.expire_blockhash();

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&super_authority)];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            kamino_init_ix.clone(),
            {
                // reserve_liquidity_mint: invalid owner
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve liquidity mint: invalid owner"),
                // kamino_reserve: invalid owner
                13 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve: invalid owner"),
                // reserve_farm_collateral: invalid owner
                14 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve farm collateral: invalid owner"),
                // market: invalid owner
                16 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino market: invalid owner"),
                // klend: modify program id
                17 => invalid_program_id(InstructionError::IncorrectProgramId, "Klend: Invalid program id"),
                // kfarms: modify program id
                18 => invalid_program_id(InstructionError::IncorrectProgramId, "Kfarms: Invalid program id"),
                // system_program: modify program id
                19 => invalid_program_id(InstructionError::IncorrectProgramId, "System program: Invalid program id"),
                // rent: modify program id
                20 => invalid_program_id(InstructionError::IncorrectProgramId, "Rent sysvar: Invalid program id"),
            }
        );

        Ok(())
    }

    /// Since Push and Pull share the same accounts validation struct,
    /// with remaining_accounts in the case of Push, we will only test for Push IX
    #[test]
    fn test_kamino_push_pull_inner_ctx_invalid_accounts_fails(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint_token_program = spl_token::ID;
        let liquidity_mint_transfer_fee = None;
        let reward_mint_token_program = spl_token::ID;
        let reward_mint_transfer_fee = None;

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
            10_000,
            true,
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

        // withdraw all liquidity deposited, which closes the Obligation
        let pull_ix = get_pull_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            u64::MAX,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &liquidity_mint_token_program,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix.clone(), pull_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        svm.expire_blockhash();

        let deposited_amount = 100_000_000;
        let mut push_ix = get_push_ix(
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

        // Checks for inner_ctx accounts:
        // (index 7) reserve_vault:
        //      owned by spl_token or token22, pubkey matches integration reserve vault
        // (index 8) obligation:
        //      pubkey matches the integration config
        // (index 9) kamino_reserve:
        //      pubkey matches the integration config and is owned by KLEND
        // (index 10) kamino_reserve_liquidity_mint
        //      pubkey matches the integration config and is owned by spl_token or Token2022
        // (index 11) kamino_reserve_liquidity_supply
        //      pubkey matches PDA and is owned by spl_token or Token2022
        // (index 12) kamino_reserve_collateral_mint:
        //      pubkey matches PDA and is owned by spl_token or Token2022
        // (index 13) kamino_reserve_collateral_supply:
        //      pubkey matches PDA and is owned by spl_token or Token2022
        // (index 14) market_authority:
        //      pubkey matches PDA
        // (index 15) kamino_market:
        //      pubkey matches the integration config and is owned by KLEND
        // (index 16) collateral_token_program:
        //      pubkey == spl_token
        // (index 17) liquidity_token_program:
        //      pubkey == spl_token/Token2022
        // (index 18) instruction_sysvar_account:
        //      pubkey == instruction sysvar acc
        // (index 19) obligation_farm_collateral:
        //      pubkey matches PDA and is owned by KFARMS or system program
        // (index 20) reserve_farm_collateral:
        //      owned by KFARMS if pubkey != pubkey::default
        // (index 21) kamino_farms_program:
        //      pubkey == KFARMS
        // (index 22) kamino_program:
        //      pubkey == KLEND
        // remaining_accounts, only for push
        // (index 24) user_metadata:
        //      pubkey matches PDA
        // (index 25) system_program:
        //      pubkey == system_program
        // (index 26) rent:
        //      pubkey == rent

        // change reserve_vault pubkey
        let reserve_vault_pk = push_ix.accounts[7].pubkey;
        let fake_reserve_vault_pk = create_account_clone_w_new_pk(&mut svm, &reserve_vault_pk);
        push_ix.accounts[7].pubkey = fake_reserve_vault_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        push_ix.accounts[7].pubkey = reserve_vault_pk;

        // change obligation pubkey
        let obligation_account_pk = push_ix.accounts[8].pubkey;
        push_ix.accounts[8].pubkey = Pubkey::new_unique();
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        push_ix.accounts[8].pubkey = obligation_account_pk;

        // change kamino_reserve pubkey
        let kamino_reserve_pk = push_ix.accounts[9].pubkey;
        let fake_kamino_reserve_pk = create_account_clone_w_new_pk(&mut svm, &kamino_reserve_pk);
        push_ix.accounts[9].pubkey = fake_kamino_reserve_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        push_ix.accounts[9].pubkey = kamino_reserve_pk;

        // change kamino_reserve_liquidity_mint pubkey
        let kamino_reserve_liq_mint_pk = push_ix.accounts[10].pubkey;
        let fake_kamino_reserve_liq_mint_pk =
            create_account_clone_w_new_pk(&mut svm, &kamino_reserve_liq_mint_pk);
        push_ix.accounts[10].pubkey = fake_kamino_reserve_liq_mint_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        push_ix.accounts[10].pubkey = kamino_reserve_liq_mint_pk;

        // change kamino_reserve_liquidity_supply pubkey
        let kamino_reserve_liq_supply_pk = push_ix.accounts[11].pubkey;
        let fake_kamino_reserve_liq_supply_pk =
            create_account_clone_w_new_pk(&mut svm, &kamino_reserve_liq_supply_pk);
        push_ix.accounts[11].pubkey = fake_kamino_reserve_liq_supply_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        push_ix.accounts[11].pubkey = kamino_reserve_liq_supply_pk;

        // change kamino_reserve_collateral_mint pubkey
        let kamino_reserve_collateral_mint_pk = push_ix.accounts[12].pubkey;
        let fake_kamino_reserve_collateral_mint_pk =
            create_account_clone_w_new_pk(&mut svm, &kamino_reserve_collateral_mint_pk);
        push_ix.accounts[12].pubkey = fake_kamino_reserve_collateral_mint_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        push_ix.accounts[12].pubkey = kamino_reserve_collateral_mint_pk;

        // change kamino_reserve_collateral_supply pubkey
        let kamino_reserve_collateral_supply_pk = push_ix.accounts[13].pubkey;
        let fake_kamino_reserve_collateral_supply_pk =
            create_account_clone_w_new_pk(&mut svm, &kamino_reserve_collateral_supply_pk);
        push_ix.accounts[13].pubkey = fake_kamino_reserve_collateral_supply_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        push_ix.accounts[13].pubkey = kamino_reserve_collateral_supply_pk;

        // change market_authority pubkey
        let market_authority_pk = push_ix.accounts[14].pubkey;
        push_ix.accounts[14].pubkey = Pubkey::new_unique();
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        push_ix.accounts[14].pubkey = market_authority_pk;

        // change kamino market pubkey
        let kamino_market_pk = push_ix.accounts[15].pubkey;
        let fake_kamino_market_pk = create_account_clone_w_new_pk(&mut svm, &kamino_market_pk);
        push_ix.accounts[15].pubkey = fake_kamino_market_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        push_ix.accounts[15].pubkey = kamino_market_pk;

        // change obligation_farm_collateral pubkey
        let obligation_farm_collateral_pk = push_ix.accounts[19].pubkey;
        let fake_obligation_farm_collateral_pk =
            create_account_clone_w_new_pk(&mut svm, &obligation_farm_collateral_pk);
        push_ix.accounts[19].pubkey = fake_obligation_farm_collateral_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        push_ix.accounts[19].pubkey = obligation_farm_collateral_pk;

        // change user_metadata pubkey
        let user_metadata_pk = push_ix.accounts[24].pubkey;
        let fake_user_metadata_pk = create_account_clone_w_new_pk(&mut svm, &user_metadata_pk);
        push_ix.accounts[24].pubkey = fake_user_metadata_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), push_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        push_ix.accounts[24].pubkey = user_metadata_pk;

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&super_authority)];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            push_ix.clone(),
            {
                // reserve_vault: invalid owner
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve vault: invalid owner"),
                // kamino_reserve: invalid owner
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve: invalid owner"),
                // kamino_reserve_liquidity_mint: invalid owner
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve liquidity mint: invalid owner"),
                // kamino_reserve_liquidity_supply: invalid owner
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve liquidity supply: invalid owner"),
                // kamino_reserve_collateral_mint: invalid owner
                12 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve collateral mint: invalid owner"),
                // kamino_reserve_collateral_supply: invalid owner
                13 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve collateral supply: invalid owner"),
                // kamino_market: invalid owner
                15 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino market: invalid owner"),
                // collateral_token_program: modify program id
                16 => invalid_program_id(InstructionError::IncorrectProgramId, "Collateral token program: Invalid program id"),
                // liquidity_token_program: modify program id
                17 => invalid_program_id(InstructionError::IncorrectProgramId, "Liquidity token program: Invalid program id"),
                // instruction_sysvar_account: modify program id
                18 => invalid_program_id(InstructionError::IncorrectProgramId, "Instruction sysvar: Invalid program id"),
                // obligation_farm_collateral: invalid owned
                19 => invalid_owner(InstructionError::InvalidAccountOwner, "Obligation farm collateral: invalid owner"),
                // reserve_farm_collateral: invalid owner
                20 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve farm collateral: invalid owner"),
                // kamino_farms_program: modify program id
                21 => invalid_program_id(InstructionError::IncorrectProgramId, "Kfarms: Invalid program id"),
                // kamino_program: modify program id
                22 => invalid_program_id(InstructionError::IncorrectProgramId, "Klend: Invalid program id"),
                // system_program: modify program id
                25 => invalid_program_id(InstructionError::IncorrectProgramId, "System program: Invalid program id"),
                // rent: modify program id
                26 => invalid_program_id(InstructionError::IncorrectProgramId, "Rent sysvar: Invalid program id"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_kamino_sync_inner_ctx_invalid_fails() -> Result<(), Box<dyn std::error::Error>> {
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
            &spl_token::ID,
            None,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &spl_token::ID,
            &liquidity_mint,
            &spl_token::ID,
            10_000,
            true,
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
            &liquidity_mint,
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
            &spl_token::ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        edit_ata_amount(
            &mut svm,
            &controller_authority,
            &kamino_config.reserve_liquidity_mint,
            1_100_000_000_000,
        )?;

        let obligation_collateral_farm =
            derive_obligation_farm_address(&reserve_context.reserve_farm_collateral, &obligation);

        // increase unclaimed rewards of obligation farm
        let rewards_unclaimed = 100_000_000;
        set_obligation_farm_rewards_issued_unclaimed(
            &mut svm,
            &obligation_collateral_farm,
            &reward_mint,
            &spl_token::ID,
            rewards_unclaimed,
        )?;

        let harvest_acounts = HarvestRewardAccounts {
            rewards_mint: &reward_mint,
            global_config: &farms_context.global_config,
            reserve_farm_collateral: &reserve_context.reserve_farm_collateral,
            scope_prices: &KAMINO_FARMS_PROGRAM_ID,
            rewards_token_program: &spl_token::ID,
        };

        let mut sync_ix = create_sync_kamino_lend_ix(
            &controller_pk,
            &integration_pk,
            &super_authority.pubkey(),
            &kamino_config,
            &spl_token::ID,
            Some(harvest_acounts),
        );
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

        // Checks for inner_ctx accounts:
        // (index 5) reserve_vault:
        //      owned by spl_token or token22, pubkey matches integration config
        // (index 6) kamino_reserve:
        //      owned by klend, pubkey matches integration config
        // (index 7) obligation:
        //      owned by klend, pubkey matches integration config
        // (index 8) obligation farm:
        //      pubkey matches PDA and is owned by KFARMS IF kamino_reserve_farm is not Pubkey::default()
        // (index 9) kamino_reserve_farm:
        //      if not Pubkey::default(), owned by KFARMS
        // (index 10) rewards_vault
        //      pubkey matches PDA and is owned by spl_token or Token2022 IF kamino_reserve_farm is not Pubkey::default()
        // (index 11) rewards_treasury_vault
        //      same as rewards_vault
        // (index 12) farm_vaults_authority
        //      pubkey matches PDA IF kamino_reserve_farm is not Pubkey::default()
        // (index 13) farms_global_config:
        //      owned by KFARMS
        // (index 14) rewards_ata:
        //      owned by spl_token or token2022 if not Pubkey::default() and IF kamino_reserve_farm is not Pubkey::default()
        // (index 15) rewards_mint:
        //      owned by spl_token or token2022 IF kamino_reserve_farm is not Pubkey::default()
        // (index 17) rewards_token_program:
        //      pubkey == spl_token or token2022
        // (index 18) kamino_farms_program:
        //      pubkey == spl_token or token2022
        // (index 19) system_program:
        //      pubkey == associated_token_program
        // (index 20) associated_token_program:
        //      pubkey == associated_token_program program

        // change reserve_vault pubkey
        let reserve_vault_pk = sync_ix.accounts[5].pubkey;
        let fake_reserve_vault_pk = create_account_clone_w_new_pk(&mut svm, &reserve_vault_pk);
        sync_ix.accounts[5].pubkey = fake_reserve_vault_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), sync_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        sync_ix.accounts[5].pubkey = reserve_vault_pk;

        // change kamino_reserve pubkey
        let kamino_reserve_pk = sync_ix.accounts[6].pubkey;
        let fake_kamino_reserve_pk = create_account_clone_w_new_pk(&mut svm, &kamino_reserve_pk);
        sync_ix.accounts[6].pubkey = fake_kamino_reserve_pk;
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), sync_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        sync_ix.accounts[6].pubkey = kamino_reserve_pk;

        // change obligation pubkey
        let obligation_account_pk = sync_ix.accounts[7].pubkey;
        sync_ix.accounts[7].pubkey =
            create_account_clone_w_new_pk(&mut svm, &obligation_account_pk);
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), sync_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );
        sync_ix.accounts[7].pubkey = obligation_account_pk;

        // change obligation_farm pubkey
        let obligation_farm_pk = sync_ix.accounts[8].pubkey;
        sync_ix.accounts[8].pubkey = create_account_clone_w_new_pk(&mut svm, &obligation_farm_pk);
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), sync_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        sync_ix.accounts[8].pubkey = obligation_farm_pk;

        // change rewards_vault pubkey
        let rewards_vault_pk = sync_ix.accounts[10].pubkey;
        sync_ix.accounts[10].pubkey = create_account_clone_w_new_pk(&mut svm, &rewards_vault_pk);
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), sync_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        sync_ix.accounts[10].pubkey = rewards_vault_pk;

        // change rewards_treasury_vault pubkey
        let rewards_treasury_vault_pk = sync_ix.accounts[11].pubkey;
        sync_ix.accounts[11].pubkey =
            create_account_clone_w_new_pk(&mut svm, &rewards_treasury_vault_pk);
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), sync_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        sync_ix.accounts[11].pubkey = rewards_treasury_vault_pk;

        // change farm_vaults_authority pubkey
        let farm_vaults_authority_pk = sync_ix.accounts[12].pubkey;
        sync_ix.accounts[12].pubkey = Pubkey::new_unique();
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[cu_ix.clone(), sync_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ));
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::InvalidPda);
        sync_ix.accounts[12].pubkey = farm_vaults_authority_pk;

        let signers: Vec<Box<&dyn solana_sdk::signer::Signer>> = vec![Box::new(&super_authority)];
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            signers,
            sync_ix.clone(),
            {
                // reserve_vault: invalid owner
                5 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve vault: invalid owner"),
                // kamino_reserve: invalid owner
                6 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve: invalid owner"),
                // obligation: invalid owner
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "Obligation: invalid owner"),
                // obligation_farm: invalid owner
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Obligation farm: invalid owner"),
                // kamino_reserve_farm: invalid owner
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Kamino reserve farm: invalid owner"),
                // rewards vault: invalid owner
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Rewards vault: invalid owner"),
                // rewards_treasury_vault: invalid owner
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Rewards treasury vault: invalid owner"),
                // farms_global_config: invalid owner
                13 => invalid_owner(InstructionError::InvalidAccountOwner, "Farms global config: invalid owner"),
                // rewards_ata: invalid owner
                14 => invalid_owner(InstructionError::InvalidAccountOwner, "Rewards ata: invalid owner"),
                // rewards_mint: invalid owner
                15 => invalid_owner(InstructionError::InvalidAccountOwner, "Rewards mint: invalid owner"),
                // rewards_token_program: modify program id
                17 => invalid_program_id(InstructionError::IncorrectProgramId, "Rewards token program: Invalid program id"),
                // kamino_farms_program: modify program id
                18 => invalid_program_id(InstructionError::IncorrectProgramId, "kamino farms program: Invalid program id"),
                // system_program: modify program id
                19 => invalid_program_id(InstructionError::IncorrectProgramId, "System program: Invalid program id"),
                // associated_token_program: modify program id
                20 => invalid_program_id(InstructionError::IncorrectProgramId, "Associated token program: Invalid program id"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_kamino_sync_fails_on_stale_reserve() -> Result<(), Box<dyn std::error::Error>> {
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
            &spl_token::ID,
            None,
        )?;

        let KaminoTestContext {
            lending_market,
            reserve_context,
            farms_context,
        } = setup_kamino_state(
            &mut svm,
            &liquidity_mint,
            &spl_token::ID,
            &liquidity_mint,
            &spl_token::ID,
            10_000,
            true,
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
            &liquidity_mint,
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

        // Deposit some amount into kamino
        let push_amount = 100_000_000;
        let push_ix = get_push_ix(
            &mut svm,
            &controller_pk,
            &super_authority,
            &integration_pk,
            &obligation,
            &kamino_config,
            push_amount,
            &Pubkey::default(),
            &reserve_context.reserve_farm_collateral,
            &spl_token::ID,
        )?;
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // Advance the slot to make the reserve stale
        let clock = svm.get_sysvar::<Clock>();
        svm.warp_to_slot(clock.slot + 2); // Advance by 2 slots to make slots_elapsed >= STALE_AFTER_SLOTS_ELAPSED

        // Debug: Check the new slot
        let new_clock = svm.get_sysvar::<Clock>();
        println!("New slot after warp: {}", new_clock.slot);

        // Refresh the kamino reserve and obligation before sync (like other tests do)
        refresh_kamino_reserve(
            &mut svm,
            &super_authority,
            &kamino_config.reserve,
            &kamino_config.market,
            &KAMINO_FARMS_PROGRAM_ID,
        )?;

        refresh_kamino_obligation(
            &mut svm,
            &super_authority,
            &kamino_config.market,
            &obligation,
            vec![&kamino_config.reserve],
        )?;

        // Now manipulate the Kamino reserve to make it stale AFTER refresh
        // We need to modify the last_update field to make it stale
        let kamino_reserve_account = svm
            .get_account(&kamino_config.reserve)
            .expect("Failed to fetch kamino reserve");

        // Parse the current reserve data
        let reserve_data = &kamino_reserve_account.data[8..]; // Skip discriminator
        let kamino_reserve: &KaminoReserve =
            bytemuck::try_from_bytes(reserve_data).map_err(|_| "Failed to parse KaminoReserve")?;

        // Create a new LastUpdate with stale data using bytemuck
        // Since LastUpdate fields are private, we'll create it from raw bytes
        let stale_slot = 0u64; // Set to 0, but we'll make current_slot higher
        let stale_flag = 1u8; // Set stale flag to true
        let price_status = 0u8; // Set price status to 0 (no flags set)
        let padding = [0u8; 6];

        // Create the LastUpdate struct from raw bytes
        let mut last_update_bytes = Vec::new();
        last_update_bytes.extend_from_slice(&stale_slot.to_le_bytes());
        last_update_bytes.extend_from_slice(&stale_flag.to_le_bytes());
        last_update_bytes.extend_from_slice(&price_status.to_le_bytes());
        last_update_bytes.extend_from_slice(&padding);

        let stale_last_update: LastUpdate = *bytemuck::try_from_bytes(&last_update_bytes)
            .map_err(|_| "Failed to create LastUpdate from bytes")?;

        // Create a new KaminoReserve with the stale last_update
        let mut modified_kamino_reserve = *kamino_reserve;
        modified_kamino_reserve.last_update = stale_last_update;

        // Serialize the modified reserve data back
        let mut modified_data = Vec::new();
        modified_data.extend_from_slice(&KaminoReserve::DISCRIMINATOR);
        modified_data.extend_from_slice(bytemuck::bytes_of(&modified_kamino_reserve));

        // Update the account with the modified data
        let mut modified_account = kamino_reserve_account.clone();
        modified_account.data = modified_data;
        svm.set_account(kamino_config.reserve, modified_account)?;

        // Initialize rewards ATA for harvesting
        let _rewards_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &controller_authority,
            &liquidity_mint,
        )?;

        let harvest_accounts = HarvestRewardAccounts {
            rewards_mint: &liquidity_mint,
            global_config: &farms_context.global_config,
            reserve_farm_collateral: &reserve_context.reserve_farm_collateral,
            scope_prices: &KAMINO_FARMS_PROGRAM_ID,
            rewards_token_program: &spl_token::ID,
        };

        // Now try to sync - it should fail because the reserve is stale
        let sync_ix = create_sync_kamino_lend_ix(
            &controller_pk,
            &integration_pk,
            &super_authority.pubkey(),
            &kamino_config,
            &spl_token::ID,
            Some(harvest_accounts),
        );
        let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let tx = Transaction::new_signed_with_payer(
            &[cu_ix, sync_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);

        // Verify that the transaction failed with the expected error
        assert!(tx_result.is_err());
        let error = tx_result.err().unwrap();
        assert_eq!(
            error.err,
            TransactionError::InstructionError(1, InstructionError::InvalidAccountData)
        );

        Ok(())
    }
}
