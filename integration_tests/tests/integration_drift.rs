mod helpers;
mod subs;

#[cfg(test)]
mod tests {

    use std::u64;

    use crate::helpers::assert::assert_custom_error;
    use crate::helpers::drift::state::spot_market::setup_drift_spot_market_vault;
    use crate::helpers::drift::{
        advance_clock_1_drift_year_to_accumulate_interest, set_drift_spot_market_pool_id,
        spot_market_accrue_cumulative_interest,
    };
    use crate::helpers::pyth::oracle::setup_mock_oracle_account;
    use crate::subs::{fetch_reserve_account, get_mint, get_token_balance_or_zero};
    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{
            drift::{set_drift_spot_market, setup_drift_state, User, UserStats},
            setup_test_controller, TestContext,
        },
        subs::{
            airdrop_lamports, fetch_integration_account, initialize_mint, initialize_reserve,
            manage_permission, mint_tokens,
        },
        test_invalid_accounts,
    };
    use borsh::BorshDeserialize;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signer::keypair::Keypair;
    use solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use spl_token;
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::integrations::drift::{
        derive_spot_market_pda, get_inner_remaining_accounts,
    };
    use svm_alm_controller_client::pull::drift::create_drift_pull_instruction;
    use svm_alm_controller_client::{
        derive_controller_authority_pda,
        generated::types::{
            AccountingAction, AccountingDirection, AccountingEvent, DriftConfig, IntegrationConfig,
            IntegrationState, IntegrationStatus, IntegrationUpdateEvent, PermissionStatus,
            ReserveStatus, SvmAlmControllerEvent,
        },
        initialize_integration::create_drift_initialize_integration_instruction,
        instructions::create_drift_push_instruction,
        integrations::drift::{derive_user_pda, derive_user_stats_pda},
        sync_integration::create_drift_sync_integration_instruction,
    };
    use test_case::test_case;

    #[test]
    fn initialize_drift_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
            None,
        )?;

        let spot_market_index = 0;
        let oracle_price = 100;
        let pool_id = 1;
        setup_drift_state(&mut svm);
        set_drift_spot_market(&mut svm, spot_market_index, &mint, oracle_price, pool_id);
        set_drift_spot_market(
            &mut svm,
            spot_market_index + 1,
            &mint,
            oracle_price,
            pool_id,
        );

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm
            .send_transaction(tx.clone())
            .map_err(|e| e.meta.pretty_logs())?;

        let clock = svm.get_sysvar::<Clock>();

        let integration = fetch_integration_account(&svm, &integration_pubkey)
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

        match &integration.config {
            IntegrationConfig::Drift(c) => {
                assert_eq!(
                    c,
                    &DriftConfig {
                        sub_account_id,
                        spot_market_index,
                        pool_id,
                        padding: [0u8; 219]
                    }
                )
            }
            _ => panic!("invalid config"),
        };

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        // Assert UserStats created and authority is controller_authority
        let drift_user_stats_pda = derive_user_stats_pda(&controller_authority);
        let drift_user_stats_acct = svm.get_account(&drift_user_stats_pda).unwrap();
        let drift_user_stats = UserStats::try_from(&drift_user_stats_acct.data).unwrap();
        assert_eq!(drift_user_stats.authority, controller_authority);

        // Assert User created
        let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
        let drift_user_acct = svm.get_account(&drift_user_pda).unwrap();
        let drift_user = User::try_from(&drift_user_acct.data).unwrap();
        assert_eq!(drift_user.authority, controller_authority);
        assert_eq!(drift_user.sub_account_id, sub_account_id);
        assert_eq!(drift_user.total_deposits, 0);
        assert_eq!(drift_user.total_withdraws, 0);

        // Assert emitted event
        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: integration_pubkey,
            authority: super_authority.pubkey(),
            old_state: None,
            new_state: Some(integration),
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            expected_event
        );

        // Creation of a second Integraiton should work without error
        // due to checks UserStats and User exist.
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            // Increment spot market index so integration key is different
            spot_market_index + 1,
            pool_id,
        );
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);
        assert!(tx_result.is_ok());

        Ok(())
    }

    #[test]
    fn initialize_drift_invalid_pool_id_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
            None,
        )?;

        let spot_market_index = 0;
        let oracle_price = 100;
        let pool_id = 1;
        setup_drift_state(&mut svm);
        set_drift_spot_market(&mut svm, spot_market_index, &mint, oracle_price, pool_id);
        set_drift_spot_market(
            &mut svm,
            spot_market_index + 1,
            &mint,
            oracle_price,
            pool_id,
        );

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone());
        assert!(tx_result.is_ok());

        // Creation of a second Integraton should error if the spot_market pool_id is changed
        // to a different value. Same drift user since sub_account_id is unchanged.

        set_drift_spot_market_pool_id(&mut svm, &derive_spot_market_pda(spot_market_index + 1), 2);

        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            // Increment spot market index so integration key is different
            spot_market_index + 1,
            pool_id,
        );
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx);
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );

        Ok(())
    }

    #[test]
    fn initiailize_drift_invalid_spot_market_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
            None,
        )?;

        setup_drift_state(&mut svm);

        let spot_market_index = 0;
        let oracle_price = 100;
        let pool_id = 0;
        let spot_market = set_drift_spot_market(&mut svm, 0, &mint, oracle_price, pool_id);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );

        let valid_market = svm.get_account(&spot_market.pubkey).unwrap();

        // overwrite with incorrect market ID
        let mut invalid_market = valid_market.clone();
        invalid_market.data[684..686].copy_from_slice(&9u16.to_le_bytes());
        svm.set_account(spot_market.pubkey, invalid_market).unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone());
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        svm.set_account(spot_market.pubkey, valid_market.clone())
            .unwrap();
        svm.expire_blockhash();

        // overwrite with incorrect mint
        let mut invalid_market = valid_market.clone();
        invalid_market.data[72..104].copy_from_slice(Pubkey::new_unique().as_ref());
        svm.set_account(spot_market.pubkey, invalid_market).unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone());
        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
        svm.set_account(spot_market.pubkey, valid_market).unwrap();
        svm.expire_blockhash();

        Ok(())
    }

    #[test]
    fn initiailize_drift_bad_token_extension_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token_2022::ID,
            None,
            Some(true),
        )?;

        setup_drift_state(&mut svm);

        let spot_market_index = 0;
        let oracle_price = 100;
        let pool_id = 0;
        set_drift_spot_market(&mut svm, 0, &mint, oracle_price, pool_id);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone());
        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::InvalidTokenMintExtension,
        );

        Ok(())
    }

    #[test_case(spl_token::ID, None ; "SPL Token Program")]
    #[test_case(spl_token_2022::ID, Some(0) ; "Token2022 Program")]
    fn drift_push_success(
        token_program: Pubkey,
        transfer_fee_bps: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();
        let oracle_price = 100;

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &token_program,
            transfer_fee_bps,
            None,
        )?;
        let pool_id = 0;
        let spot_market = set_drift_spot_market(
            &mut svm,
            spot_market_index,
            &token_mint,
            oracle_price,
            pool_id,
        );

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &token_program);

        // Set up mock oracle and insurance fund accounts
        setup_mock_oracle_account(&mut svm, &spot_market.oracle, oracle_price);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &token_program,
        )?;

        // Create associated token account for controller authority and mint tokens
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Create the push instruction
        let push_amount = 100_000_000;

        let integration_before = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .expect("reserve should exist")
            .unwrap();

        // Get initial token account balances
        let reserve_vault_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        let spot_market_vault_before = get_token_balance_or_zero(&svm, &spot_market.vault);

        // Fetch drift user state before push
        let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
        let drift_user_acct_before = svm.get_account(&drift_user_pda).unwrap();
        let drift_user_before = User::try_from(&drift_user_acct_before.data).unwrap();

        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &token_program,
            spot_market_index,
            sub_account_id,
            push_amount,
            &inner_remaining_accounts,
        )?;

        // Execute the push instruction
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).unwrap();

        let integration_after = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .expect("reserve should exist")
            .unwrap();

        // Get final token account balances
        let reserve_vault_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        let spot_market_vault_after = get_token_balance_or_zero(&svm, &spot_market.vault);

        // Fetch drift user state after push
        let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
        let drift_user_acct_after = svm.get_account(&drift_user_pda).unwrap();
        let drift_user_after = User::try_from(&drift_user_acct_after.data).unwrap();

        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_before.rate_limit_outflow_amount_available - push_amount
        );

        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_before.rate_limit_outflow_amount_available - push_amount
        );

        // Assert reserve vault balance decreased by push amount
        assert_eq!(
            reserve_vault_after,
            reserve_vault_before - push_amount,
            "Reserve vault should have decreased by push amount"
        );

        // Assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::Drift(state) => state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::Drift(state) => state,
            _ => panic!("invalid state"),
        };

        // Assert Drift integration state update balance by push amount
        assert_eq!(
            state_after.balance,
            state_before.balance + push_amount,
            "Drift integration state balance should have increased by push amount"
        );

        // Assert spot market vault balance increased by push amount
        assert_eq!(
            spot_market_vault_after,
            spot_market_vault_before + push_amount,
            "Drift spot market vault should have increased by push amount"
        );

        // Find the spot position for the market we're depositing into
        let spot_position_index = drift_user_after
            .spot_positions
            .iter()
            .position(|pos| pos.market_index == spot_market_index)
            .expect("Spot position should exist for the market");

        let spot_position_before = drift_user_before.spot_positions[spot_position_index];
        let spot_position_after = drift_user_after.spot_positions[spot_position_index];

        // Assert spot position cumulative_deposits increased by push amount
        assert_eq!(
            spot_position_after.cumulative_deposits,
            spot_position_before.cumulative_deposits + push_amount as i64,
            "Spot position cumulative_deposits should have increased by push amount"
        );

        // Copy packed field to avoid unaligned reference error
        let cumulative_deposit_interest = spot_market.cumulative_deposit_interest;

        let token_mint_account = get_mint(&svm, &token_mint);
        // https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L45
        let spot_balance_precision = 10_u128.pow(19 - token_mint_account.decimals as u32); // 10^13 (19 - 6)
        let expected_scaled_balance_increase = (push_amount as u128
            * spot_balance_precision as u128
            / cumulative_deposit_interest) as u64;

        // Assert spot position scaled_balance increased by the calculated amount
        assert_eq!(
            spot_position_after.scaled_balance,
            spot_position_before.scaled_balance + expected_scaled_balance_increase,
            "Spot position scaled_balance should have increased by calculated amount based on cumulative deposit interest"
        );

        // Assert the spot position balance_type is 0 (Deposit)
        assert_eq!(
            spot_position_after.balance_type, 0,
            "Spot position balance_type should be 0 (Deposit)"
        );

        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey),
                mint: token_mint,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Deposit,
                delta: push_amount,
            })
        );

        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: None,
                mint: token_mint,
                reserve: Some(reserve_keys.pubkey),
                direction: AccountingDirection::Debit,
                action: AccountingAction::Deposit,
                delta: push_amount,
            })
        );

        Ok(())
    }

    #[test_case(spl_token::ID, None ; "SPL Token Program")]
    #[test_case(spl_token_2022::ID, Some(0) ; "Token2022 Program")]
    fn drift_sync_integration_success(
        token_program: Pubkey,
        transfer_fee_bps: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();
        let oracle_price = 100;

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &token_program,
            transfer_fee_bps,
            None,
        )?;

        let pool_id = 0;
        let spot_market = set_drift_spot_market(
            &mut svm,
            spot_market_index,
            &token_mint,
            oracle_price,
            pool_id,
        );

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &token_program);

        setup_mock_oracle_account(&mut svm, &spot_market.oracle, oracle_price);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &token_program,
        )?;

        // Create associated token account for controller authority and mint tokens
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Push some tokens to drift first to have something to sync
        let push_amount = 100_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &token_program,
            spot_market_index,
            sub_account_id,
            push_amount,
            &inner_remaining_accounts,
        )?;

        // Execute the push instruction
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // Create the sync instruction
        let sync_ix = create_drift_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
            &reserve_keys.pubkey,
            &spot_market.oracle,
            spot_market_index,
            sub_account_id,
        )?;

        // Accrue 1% of interest for deposits
        spot_market_accrue_cumulative_interest(&mut svm, spot_market_index, 100);
        advance_clock_1_drift_year_to_accumulate_interest(&mut svm);

        // Execute the sync instruction
        let tx = Transaction::new_signed_with_payer(
            &[sync_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).unwrap();

        // Get integration state after sync
        let integration_after = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        // The balance should reflect the pushed amount plus interest earned
        // With 1% interest, the balance should be approximately push_amount * 1.0
        let expected_interest = push_amount.checked_div(100).unwrap();
        let expected_total_balance = push_amount + expected_interest;
        // Verify that the integration state was updated with interest
        match &integration_after.state {
            IntegrationState::Drift(drift_state) => {
                assert_eq!(drift_state.balance, expected_total_balance);
            }
            _ => panic!("Expected Drift integration state"),
        }

        // Assert the sync event from shared_sync.rs - this should use the token_mint, not reserve_keys.pubkey
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey),
                mint: token_mint,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Sync,
                delta: expected_interest,
            })
        );

        Ok(())
    }

    #[test_case(spl_token::ID, None ; "SPL Token Program")]
    #[test_case(spl_token_2022::ID, Some(0) ; "Token2022 Program")]
    fn drift_pull_success(
        token_program: Pubkey,
        transfer_fee_bps: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();
        let oracle_price = 100;

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &token_program,
            transfer_fee_bps,
            None,
        )?;

        let pool_id = 0;
        let spot_market = set_drift_spot_market(
            &mut svm,
            spot_market_index,
            &token_mint,
            oracle_price,
            pool_id,
        );

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &token_program);

        setup_mock_oracle_account(&mut svm, &spot_market.oracle, oracle_price);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &token_program,
        )?;

        // Create associated token account for controller authority and mint tokens
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        let amount = 100_000_000;

        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &token_program,
            spot_market_index,
            sub_account_id,
            amount,
            &inner_remaining_accounts,
        )?;

        // Execute the push instruction
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // Accrue 1% of interest for deposits
        spot_market_accrue_cumulative_interest(&mut svm, spot_market_index, 100);
        advance_clock_1_drift_year_to_accumulate_interest(&mut svm);

        let pull_ix = create_drift_pull_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &token_program,
            spot_market_index,
            sub_account_id,
            u64::MAX,
            &inner_remaining_accounts,
        )?;

        // Get initial token account balances
        let reserve_vault_before = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        let spot_market_vault_before = get_token_balance_or_zero(&svm, &spot_market.vault);

        let integration_before = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        let reserve_before = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .expect("reserve should exist")
            .unwrap();

        // Execute the pull instruction
        let tx = Transaction::new_signed_with_payer(
            &[pull_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).unwrap();

        let integration_after = fetch_integration_account(&svm, &integration_pubkey)
            .expect("integration should exist")
            .unwrap();

        let reserve_after = fetch_reserve_account(&svm, &reserve_keys.pubkey)
            .expect("reserve should exist")
            .unwrap();

        // Get final token account balances
        let reserve_vault_after = get_token_balance_or_zero(&svm, &reserve_keys.vault);
        let spot_market_vault_after = get_token_balance_or_zero(&svm, &spot_market.vault);

        // Fetch drift user state after push
        let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
        let drift_user_acct_after = svm.get_account(&drift_user_pda).unwrap();
        let drift_user_after = User::try_from(&drift_user_acct_after.data).unwrap();

        let interest = 1_000_000;
        let amount_with_interest = amount + interest;

        assert_eq!(
            integration_after.rate_limit_outflow_amount_available,
            integration_before.rate_limit_outflow_amount_available + amount
        );

        // Verify that the integration state was updated with interest
        match &integration_after.state {
            IntegrationState::Drift(drift_state) => {
                assert_eq!(drift_state.balance, 0);
            }
            _ => panic!("Expected Drift integration state"),
        }

        assert_eq!(
            reserve_after.rate_limit_outflow_amount_available,
            reserve_before.rate_limit_outflow_amount_available + amount
        );

        // Assert reserve vault balance increased by amount
        assert_eq!(
            reserve_vault_after,
            reserve_vault_before + amount_with_interest,
            "Reserve vault should have increased by amount"
        );

        // Assert spot market vault balance decreased by amount
        assert_eq!(
            spot_market_vault_before - spot_market_vault_after,
            amount_with_interest,
            "Drift spot market vault should have decreased by amount"
        );

        // Find the spot position for the market we're depositing into
        let spot_position_index = drift_user_after
            .spot_positions
            .iter()
            .position(|pos| pos.market_index == spot_market_index)
            .expect("Spot position should exist for the market");

        let spot_position_after = drift_user_after.spot_positions[spot_position_index];

        // Assert spot position balance went back to 0
        assert_eq!(spot_position_after.scaled_balance, 0);

        // Assert Sync Event emitted
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                mint: spot_market.mint,
                integration: Some(integration_pubkey),
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Sync,
                delta: interest,
            })
        );

        // Assert Integration/Reserve withdraw events
        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey),
                mint: spot_market.mint,
                reserve: None,
                direction: AccountingDirection::Debit,
                action: AccountingAction::Withdrawal,
                delta: amount_with_interest,
            })
        );

        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: None,
                mint: spot_market.mint,
                reserve: Some(reserve_keys.pubkey),
                direction: AccountingDirection::Credit,
                action: AccountingAction::Withdrawal,
                delta: amount_with_interest,
            })
        );

        Ok(())
    }

    #[test]
    fn drift_push_multiple_spot_markets_and_sub_ids_success(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        setup_drift_state(&mut svm);

        // Create two additional token mints for two additional reserves
        let token_mint_1_kp = Keypair::new();
        let token_mint_1 = token_mint_1_kp.pubkey();
        let mint_authority_1 = Keypair::new();

        let token_mint_2_kp = Keypair::new();
        let token_mint_2 = token_mint_2_kp.pubkey();
        let mint_authority_2 = Keypair::new();
        let oracle_price = 100;

        // Initialize the first additional token mint
        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority_1.pubkey(),
            None,
            6,
            Some(token_mint_1_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        // Initialize the second additional token mint
        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority_2.pubkey(),
            None,
            6,
            Some(token_mint_2_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        // Set up spot markets for the additional tokens
        let spot_market_index_1 = 0;
        let spot_market_index_2 = 1;
        let pool_id = 0;

        let spot_market_1 = set_drift_spot_market(
            &mut svm,
            spot_market_index_1,
            &token_mint_1,
            oracle_price,
            pool_id,
        );
        let spot_market_2 = set_drift_spot_market(
            &mut svm,
            spot_market_index_2,
            &token_mint_2,
            oracle_price,
            pool_id,
        );

        setup_drift_spot_market_vault(&mut svm, spot_market_index_1, &token_mint_1, &spl_token::ID);
        setup_drift_spot_market_vault(&mut svm, spot_market_index_2, &token_mint_2, &spl_token::ID);

        // Set up mock oracle accounts for both spot markets
        setup_mock_oracle_account(&mut svm, &spot_market_1.oracle, oracle_price);
        setup_mock_oracle_account(&mut svm, &spot_market_2.oracle, oracle_price);

        // Set up User accounts with spot positions for both markets
        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let pool_id = 0;

        // Define sub account IDs
        let sub_account_id_1 = 0;
        let sub_account_id_2 = 1;

        // Initialize Drift Integration for first spot market
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;

        let init_ix_1 = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint_1,
            "Drift Lend 1",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id_1,
            spot_market_index_1,
            pool_id,
        );
        let integration_pubkey_1 = init_ix_1.accounts[5].pubkey;

        let tx = Transaction::new_signed_with_payer(
            &[init_ix_1],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize Drift Integration for second spot market
        let init_ix_2 = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint_2,
            "Drift Lend 2",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id_2,
            spot_market_index_2,
            pool_id,
        );
        let integration_pubkey_2 = init_ix_2.accounts[5].pubkey;

        let tx = Transaction::new_signed_with_payer(
            &[init_ix_2],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize reserves for both tokens
        let reserve_keys_1 = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint_1,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        let reserve_keys_2 = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint_2,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Mint tokens to controller authority for both reserves
        let vault_start_amount = 1_000_000_000;

        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority_1,
            &token_mint_1,
            &controller_authority,
            vault_start_amount,
        )?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority_2,
            &token_mint_2,
            &controller_authority,
            vault_start_amount,
        )?;

        // Verify both integrations were created properly
        let integration_1 = fetch_integration_account(&svm, &integration_pubkey_1)
            .expect("integration 1 should exist")
            .unwrap();

        let integration_2 = fetch_integration_account(&svm, &integration_pubkey_2)
            .expect("integration 2 should exist")
            .unwrap();

        assert_eq!(integration_1.controller, controller_pk);
        assert_eq!(integration_1.status, IntegrationStatus::Active);
        assert_eq!(integration_2.controller, controller_pk);
        assert_eq!(integration_2.status, IntegrationStatus::Active);

        // Test push operations for both integrations
        let push_amount_1 = 100_000_000;
        let push_amount_2 = 200_000_000;

        // Push to first integration
        let inner_remaining_accounts_1 = get_inner_remaining_accounts(&[spot_market_1]);
        let push_ix_1 = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint_1,
            &integration_pubkey_1,
            &reserve_keys_1.pubkey,
            &reserve_keys_1.vault,
            &spl_token::ID,
            spot_market_index_1,
            sub_account_id_1,
            push_amount_1,
            &inner_remaining_accounts_1,
        )?;

        let tx = Transaction::new_signed_with_payer(
            &[push_ix_1],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result_1 = svm.send_transaction(tx.clone()).unwrap();

        // Push to second integration
        let inner_remaining_accounts_2 = get_inner_remaining_accounts(&[spot_market_2]);
        let push_ix_2 = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint_2,
            &integration_pubkey_2,
            &reserve_keys_2.pubkey,
            &reserve_keys_2.vault,
            &spl_token::ID,
            spot_market_index_2,
            sub_account_id_2,
            push_amount_2,
            &inner_remaining_accounts_2,
        )?;

        let tx = Transaction::new_signed_with_payer(
            &[push_ix_2],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result_2 = svm.send_transaction(tx.clone());

        let tx_result_2 = match tx_result_2 {
            Ok(tx_result) => tx_result,
            Err(e) => {
                panic!("error: {}", e.meta.pretty_logs());
            }
        };

        // Verify push operations worked correctly
        let integration_after_push_1 = fetch_integration_account(&svm, &integration_pubkey_1)
            .expect("integration 1 should exist")
            .unwrap();

        let integration_after_push_2 = fetch_integration_account(&svm, &integration_pubkey_2)
            .expect("integration 2 should exist")
            .unwrap();

        assert_eq!(
            integration_after_push_1.rate_limit_outflow_amount_available,
            integration_1.rate_limit_outflow_amount_available - push_amount_1
        );

        assert_eq!(
            integration_after_push_2.rate_limit_outflow_amount_available,
            integration_2.rate_limit_outflow_amount_available - push_amount_2
        );

        // Verify spot market vault balances increased
        let spot_market_vault_1_balance = get_token_balance_or_zero(&svm, &spot_market_1.vault);
        let spot_market_vault_2_balance = get_token_balance_or_zero(&svm, &spot_market_2.vault);

        assert_eq!(spot_market_vault_1_balance, push_amount_1);
        assert_eq!(spot_market_vault_2_balance, push_amount_2);

        // Verify reserve vault balances decreased
        let reserve_vault_1_balance = get_token_balance_or_zero(&svm, &reserve_keys_1.vault);
        let reserve_vault_2_balance = get_token_balance_or_zero(&svm, &reserve_keys_2.vault);

        assert_eq!(reserve_vault_1_balance, vault_start_amount - push_amount_1);
        assert_eq!(reserve_vault_2_balance, vault_start_amount - push_amount_2);

        // Verify accounting events were emitted for both operations
        assert_contains_controller_cpi_event!(
            tx_result_1,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey_1),
                mint: token_mint_1,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Deposit,
                delta: push_amount_1,
            })
        );

        assert_contains_controller_cpi_event!(
            tx_result_2,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey_2),
                mint: token_mint_2,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Deposit,
                delta: push_amount_2,
            })
        );

        // Update spot markets to simulate interest accrual using helper function
        spot_market_accrue_cumulative_interest(&mut svm, spot_market_index_1, 100); // 1% interest
        spot_market_accrue_cumulative_interest(&mut svm, spot_market_index_2, 200); // 2% interest
        advance_clock_1_drift_year_to_accumulate_interest(&mut svm);

        // Sync both integrations
        let sync_ix_1 = create_drift_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey_1,
            &reserve_keys_1.pubkey,
            &spot_market_1.oracle,
            spot_market_index_1,
            sub_account_id_1,
        )?;

        let sync_ix_2 = create_drift_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey_2,
            &reserve_keys_2.pubkey,
            &spot_market_2.oracle,
            spot_market_index_2,
            sub_account_id_2,
        )?;

        let tx = Transaction::new_signed_with_payer(
            &[sync_ix_1, sync_ix_2],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result_sync = svm.send_transaction(tx.clone()).unwrap();

        // Verify sync operations updated integration states
        let integration_after_sync_1 = fetch_integration_account(&svm, &integration_pubkey_1)
            .expect("integration 1 should exist")
            .unwrap();

        let integration_after_sync_2 = fetch_integration_account(&svm, &integration_pubkey_2)
            .expect("integration 2 should exist")
            .unwrap();

        // Verify integration states were updated with interest
        match &integration_after_sync_1.state {
            IntegrationState::Drift(drift_state) => {
                let expected_balance_1 = (push_amount_1 as u128)
                    .checked_mul(101)
                    .unwrap()
                    .checked_div(100)
                    .unwrap();
                assert_eq!(drift_state.balance, expected_balance_1 as u64);
            }
            _ => panic!("Expected Drift integration state for integration 1"),
        }

        match &integration_after_sync_2.state {
            IntegrationState::Drift(drift_state) => {
                let expected_balance_2 = (push_amount_2 as u128)
                    .checked_mul(102)
                    .unwrap()
                    .checked_div(100)
                    .unwrap();
                assert_eq!(drift_state.balance, expected_balance_2 as u64);
            }
            _ => panic!("Expected Drift integration state for integration 2"),
        }

        // Verify sync events were emitted
        let expected_interest_1 = (push_amount_1 as u128)
            .checked_mul(1)
            .unwrap()
            .checked_div(100)
            .unwrap();

        let expected_interest_2 = (push_amount_2 as u128)
            .checked_mul(2)
            .unwrap()
            .checked_div(100)
            .unwrap();

        assert_contains_controller_cpi_event!(
            tx_result_sync,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey_1),
                mint: token_mint_1,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Sync,
                delta: expected_interest_1 as u64,
            })
        );

        assert_contains_controller_cpi_event!(
            tx_result_sync,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey_2),
                mint: token_mint_2,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Sync,
                delta: expected_interest_2 as u64,
            })
        );

        Ok(())
    }

    #[test]
    fn drift_push_multiple_spot_positions_same_sub_id_success(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        setup_drift_state(&mut svm);

        // Create two additional token mints for two additional spot markets
        let token_mint_1_kp = Keypair::new();
        let token_mint_1 = token_mint_1_kp.pubkey();
        let mint_authority_1 = Keypair::new();

        let token_mint_2_kp = Keypair::new();
        let token_mint_2 = token_mint_2_kp.pubkey();
        let mint_authority_2 = Keypair::new();

        // Initialize the first additional token mint
        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority_1.pubkey(),
            None,
            6,
            Some(token_mint_1_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        // Initialize the second additional token mint
        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority_2.pubkey(),
            None,
            6,
            Some(token_mint_2_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        let pool_id = 0;

        // Set up spot markets for the additional tokens
        let spot_market_index_1 = 1;
        let spot_market_index_2 = 2;

        let spot_market_1 =
            set_drift_spot_market(&mut svm, spot_market_index_1, &token_mint_1, 100, pool_id);
        let spot_market_2 =
            set_drift_spot_market(&mut svm, spot_market_index_2, &token_mint_2, 100, pool_id);

        setup_drift_spot_market_vault(&mut svm, spot_market_index_1, &token_mint_1, &spl_token::ID);
        setup_drift_spot_market_vault(&mut svm, spot_market_index_2, &token_mint_2, &spl_token::ID);

        // Set up mock oracle accounts for both spot markets
        setup_mock_oracle_account(&mut svm, &spot_market_1.oracle, 100);
        setup_mock_oracle_account(&mut svm, &spot_market_2.oracle, 100);

        // Set up User account with spot positions for both markets
        let sub_account_id = 0;

        // Initialize Drift Integration for first spot market
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;

        let init_ix_1 = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint_1,
            "Drift Lend 1",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index_1,
            pool_id,
        );
        let integration_pubkey_1 = init_ix_1.accounts[5].pubkey;

        // Initialize Drift Integration for second spot market
        let init_ix_2 = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint_2,
            "Drift Lend 2",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index_2,
            pool_id,
        );
        let integration_pubkey_2 = init_ix_2.accounts[5].pubkey;

        // Initialize reserves for both tokens
        let reserve_keys_1 = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint_1,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        let reserve_keys_2 = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint_2,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Mint tokens to controller authority for both reserves
        let vault_start_amount = 1_000_000_000;

        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority_1,
            &token_mint_1,
            &controller_authority,
            vault_start_amount,
        )?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority_2,
            &token_mint_2,
            &controller_authority,
            vault_start_amount,
        )?;

        // Initialize both integrations
        let tx = Transaction::new_signed_with_payer(
            &[init_ix_1, init_ix_2],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Verify both integrations were created properly
        let integration_1 = fetch_integration_account(&svm, &integration_pubkey_1)
            .expect("integration 1 should exist")
            .unwrap();

        let integration_2 = fetch_integration_account(&svm, &integration_pubkey_2)
            .expect("integration 2 should exist")
            .unwrap();

        assert_eq!(integration_1.controller, controller_pk);
        assert_eq!(integration_1.status, IntegrationStatus::Active);
        assert_eq!(integration_2.controller, controller_pk);
        assert_eq!(integration_2.status, IntegrationStatus::Active);

        let push_amount = 200_000_000;

        let inner_remaining_accounts =
            get_inner_remaining_accounts(&[spot_market_1, spot_market_2]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint_2,
            &integration_pubkey_2,
            &reserve_keys_2.pubkey,
            &reserve_keys_2.vault,
            &spl_token::ID,
            spot_market_index_2,
            sub_account_id,
            push_amount,
            &inner_remaining_accounts,
        )?;

        // Execute both push instructions in the same transaction
        // This simulates handling multiple spot positions simultaneously
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // Verify both push operations worked correctly
        let integration_after_push_1 = fetch_integration_account(&svm, &integration_pubkey_1)
            .expect("integration 1 should exist")
            .unwrap();

        let integration_after_push_2 = fetch_integration_account(&svm, &integration_pubkey_2)
            .expect("integration 2 should exist")
            .unwrap();

        assert_eq!(
            integration_after_push_1.rate_limit_outflow_amount_available,
            integration_1.rate_limit_outflow_amount_available
        );

        assert_eq!(
            integration_after_push_2.rate_limit_outflow_amount_available,
            integration_2.rate_limit_outflow_amount_available - push_amount
        );

        // Verify spot market vault balances increased
        let spot_market_vault_1_balance = get_token_balance_or_zero(&svm, &spot_market_1.vault);
        let spot_market_vault_2_balance = get_token_balance_or_zero(&svm, &spot_market_2.vault);

        assert_eq!(spot_market_vault_1_balance, 0);
        assert_eq!(spot_market_vault_2_balance, push_amount);

        // Verify reserve vault balances decreased
        let reserve_vault_1_balance = get_token_balance_or_zero(&svm, &reserve_keys_1.vault);
        let reserve_vault_2_balance = get_token_balance_or_zero(&svm, &reserve_keys_2.vault);

        assert_eq!(reserve_vault_1_balance, vault_start_amount);
        assert_eq!(reserve_vault_2_balance, vault_start_amount - push_amount);

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
    fn test_drift_push_permissions(
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
        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        let pool_id = 0;

        let spot_market =
            set_drift_spot_market(&mut svm, spot_market_index, &token_mint, 100, pool_id);

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &spl_token::ID);

        // Set up mock oracle and insurance fund accounts
        setup_mock_oracle_account(&mut svm, &spot_market.oracle, 100);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Create associated token account for controller authority and mint tokens
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

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

        // Create the push instruction
        let push_amount = 100_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &push_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            push_amount,
            &inner_remaining_accounts,
        )?;

        // Execute the push instruction
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
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
                TransactionError::InstructionError(0, InstructionError::IncorrectAuthority)
            ),
        }
        Ok(())
    }

    #[test]
    fn drift_push_with_interest_accrual_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        let pool_id = 0;

        let spot_market =
            set_drift_spot_market(&mut svm, spot_market_index, &token_mint, 100, pool_id);

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &spl_token::ID);

        setup_mock_oracle_account(&mut svm, &spot_market.oracle, 100);
        // Set up User account with spot position for the market
        let sub_account_id = 0;

        // Initialize Drift Integration
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Create associated token account for controller authority and mint tokens
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // FIRST PUSH: Push some tokens to drift
        let first_push_amount = 100_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let first_push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            first_push_amount,
            &inner_remaining_accounts,
        )?;

        // Execute the first push instruction
        let tx = Transaction::new_signed_with_payer(
            &[first_push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // Fetch drift user state after first push to verify spot position was updated
        let drift_user_pda = derive_user_pda(&controller_authority, sub_account_id);
        let drift_user_acct_after_first_push = svm.get_account(&drift_user_pda).unwrap();
        let drift_user_after_first_push =
            User::try_from(&drift_user_acct_after_first_push.data).unwrap();

        // Find the spot position for the market we're depositing into
        let spot_position_index = drift_user_after_first_push
            .spot_positions
            .iter()
            .position(|pos| pos.market_index == spot_market_index)
            .expect("Spot position should exist for the market");

        let spot_position_after_first_push =
            drift_user_after_first_push.spot_positions[spot_position_index];

        // Assert spot position cumulative_deposits increased by first push amount
        assert_eq!(
            spot_position_after_first_push.cumulative_deposits, first_push_amount as i64,
            "Spot position cumulative_deposits should equal first push amount"
        );

        // Update the spot market to simulate interest accrual using helper function
        spot_market_accrue_cumulative_interest(&mut svm, spot_market_index, 200); // 2% interest
        advance_clock_1_drift_year_to_accumulate_interest(&mut svm);

        // SECOND PUSH: This should trigger sync_drift_balance to accrue interest
        let second_push_amount = 50_000_000;
        let inner_remaining_accounts_second = get_inner_remaining_accounts(&[spot_market]);
        let second_push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            second_push_amount,
            &inner_remaining_accounts_second,
        )?;

        // Execute the second push instruction
        let tx = Transaction::new_signed_with_payer(
            &[second_push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone()).unwrap();

        // Fetch drift user state after second push to verify spot position was updated
        let drift_user_acct_after_second_push = svm.get_account(&drift_user_pda).unwrap();
        let drift_user_after_second_push =
            User::try_from(&drift_user_acct_after_second_push.data).unwrap();

        let spot_position_after_second_push =
            drift_user_after_second_push.spot_positions[spot_position_index];

        // Calculate expected cumulative deposits: first_push_amount + second_push_amount
        let expected_cumulative_deposits = (first_push_amount + second_push_amount) as i64;

        let interest_delta = first_push_amount * 102 / 100 - first_push_amount;

        assert_contains_controller_cpi_event!(
            tx_result,
            tx.message.account_keys.as_slice(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: controller_pk,
                integration: Some(integration_pubkey),
                mint: token_mint,
                reserve: None,
                direction: AccountingDirection::Credit,
                action: AccountingAction::Sync,
                delta: interest_delta as u64,
            })
        );

        // Assert spot position cumulative_deposits increased by both push amounts
        assert_eq!(
            spot_position_after_second_push.cumulative_deposits, expected_cumulative_deposits,
            "Spot position cumulative_deposits should equal both push amounts combined"
        );

        // Verify final token balances
        let reserve_vault_final = get_token_balance_or_zero(&svm, &reserve_keys.vault);

        let expected_total = vault_start_amount - first_push_amount - second_push_amount;

        assert_eq!(
            reserve_vault_final, expected_total,
            "Reserve vault should have decreased by both push amounts plus interest accrual"
        );

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
    fn test_drift_pull_permissions(
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
        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        let pool_id = 0;

        let spot_market =
            set_drift_spot_market(&mut svm, spot_market_index, &token_mint, 100, pool_id);

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &spl_token::ID);

        // Set up mock oracle and insurance fund accounts
        setup_mock_oracle_account(&mut svm, &spot_market.oracle, 100);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Create associated token account for controller authority and mint tokens
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Push some tokens to drift first to have something to pull
        let push_amount = 100_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            push_amount,
            &inner_remaining_accounts,
        )?;

        // Execute the push instruction
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

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

        // Create the pull instruction
        let pull_amount = 50_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let pull_ix = create_drift_pull_instruction(
            &controller_pk,
            &pull_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            pull_amount,
            &inner_remaining_accounts,
        )?;

        // Execute the pull instruction
        let tx = Transaction::new_signed_with_payer(
            &[pull_ix],
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
                TransactionError::InstructionError(0, InstructionError::IncorrectAuthority)
            ),
        }
        Ok(())
    }

    #[test]
    fn drift_initialize_invalid_inner_accounts() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
            None,
        )?;

        setup_drift_state(&mut svm);

        let pool_id = 0;
        let spot_market_index = 0;
        let oracle_price = 100;
        set_drift_spot_market(&mut svm, spot_market_index, &mint, oracle_price, pool_id);

        // Create a valid drift initialize instruction
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );

        // Test invalid accounts for the inner context accounts (remaining_accounts)
        // The remaining_accounts start at index 7 (after payer, controller, controller_authority, authority, permission, integration, system_program)
        // Inner accounts are: mint, user, user_stats, state, spot_market, rent, drift_program
        test_invalid_accounts!(
            svm,
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            init_ix,
            {
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "mint: Invalid owner"),
                9 => invalid_pubkey(InstructionError::Custom(1), "Drift user: Invalid pubkey"),
                10 => invalid_pubkey(InstructionError::Custom(1), "Drift user stats: Invalid pubkey"),
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift state: Invalid owner"),
                11 => invalid_pubkey(InstructionError::Custom(1), "Drift state: Invalid pubkey"),
                12 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift spot market: Invalid owner"),
                12 => invalid_pubkey(InstructionError::Custom(1), "Drift spot market: Invalid pubkey"),
                13 => invalid_program_id(InstructionError::IncorrectProgramId, "Rent sysvar: Invalid program id"),
                14 => invalid_program_id(InstructionError::IncorrectProgramId, "Drift program: Invalid program id"),
            }
        )?;

        Ok(())
    }

    #[test]
    fn drift_push_invalid_inner_accounts() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();
        let oracle_price = 100;

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        let pool_id = 0;
        let spot_market = set_drift_spot_market(
            &mut svm,
            spot_market_index,
            &token_mint,
            oracle_price,
            pool_id,
        );

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &spl_token::ID);

        // Set up mock oracle and insurance fund accounts
        setup_mock_oracle_account(&mut svm, &spot_market.oracle, oracle_price);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Create associated token account for controller authority and mint tokens
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Create a valid drift push instruction
        let push_amount = 100_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            push_amount,
            &inner_remaining_accounts,
        )?;

        // Test invalid accounts for the inner context accounts (remaining_accounts)
        // The remaining_accounts start at index 7 (after payer, controller, controller_authority, authority, permission, integration, system_program)
        // Inner accounts are: state(7), user(8), user_stats(9), spot_market_vault(10), reserve_vault(11), token_program(12), drift_program(13)
        test_invalid_accounts!(
            svm,
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            push_ix,
            {
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift state: Invalid owner"),
                7 => invalid_pubkey(InstructionError::Custom(1), "Drift state: Invalid pubkey"),
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift user: Invalid owner"),
                8 => invalid_pubkey(InstructionError::InvalidAccountData, "Drift user: Invalid pubkey"),
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift user_stats: Invalid owner"),
                9 => invalid_pubkey(InstructionError::Custom(1), "Drift user_stats: Invalid pubkey"),
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift spot market vault: Invalid owner"),
                10 => invalid_pubkey(InstructionError::Custom(1), "Drift spot market vault: invalid pubkey"),
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve vault: Invalid owner"),
                11 => invalid_pubkey(InstructionError::InvalidAccountData, "Reserve vault: invalid pubkey"),
                12 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: Invalid program id"),
                13 => invalid_program_id(InstructionError::IncorrectProgramId, "Drift program: Invalid program id"),
            }
        )?;

        Ok(())
    }

    #[test]
    fn drift_pull_invalid_inner_accounts() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();
        let oracle_price = 100;

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        let pool_id = 0;

        let spot_market = set_drift_spot_market(
            &mut svm,
            spot_market_index,
            &token_mint,
            oracle_price,
            pool_id,
        );

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &spl_token::ID);

        // Set up mock oracle and insurance fund accounts
        setup_mock_oracle_account(&mut svm, &spot_market.oracle, oracle_price);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Create associated token account for controller authority and mint tokens
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let vault_start_amount = 1_000_000_000;

        // Mint tokens to controller authority
        mint_tokens(
            &mut svm,
            &super_authority,
            &mint_authority,
            &token_mint,
            &controller_authority,
            vault_start_amount,
        )?;

        // Push some tokens to drift first to have something to pull
        let push_amount = 100_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let push_ix = create_drift_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            push_amount,
            &inner_remaining_accounts,
        )?;

        // Execute the push instruction
        let tx = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone()).unwrap();

        // Create a valid drift pull instruction
        let pull_amount = 50_000_000;
        let inner_remaining_accounts = get_inner_remaining_accounts(&[spot_market]);
        let pull_ix = create_drift_pull_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            &integration_pubkey,
            &reserve_keys.pubkey,
            &reserve_keys.vault,
            &spl_token::ID,
            spot_market_index,
            sub_account_id,
            pull_amount,
            &inner_remaining_accounts,
        )?;

        // Test invalid accounts for the inner context accounts (remaining_accounts)
        // The remaining_accounts start at index 7 (after payer, controller, controller_authority, authority, permission, integration, system_program)
        // Inner accounts are: state(7), user(8), user_stats(9), spot_market_vault(10), drift_signer(11), reserve_vault(12), token_program(13), drift_program(14)
        test_invalid_accounts!(
            svm,
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            pull_ix,
            {
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift state: Invalid owner"),
                7 => invalid_pubkey(InstructionError::Custom(1), "Drift state: Invalid pubkey"),
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift user: Invalid owner"),
                8 => invalid_pubkey(InstructionError::InvalidAccountData, "Drift user: Invalid pubkey"),
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift user_stats: Invalid owner"),
                9 => invalid_pubkey(InstructionError::Custom(1), "Drift user_stats: Invalid pubkey"),
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift spot market vault: Invalid owner"),
                10 => invalid_pubkey(InstructionError::Custom(1), "Drift spot market vault: Invalid pubkey"),
                12 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve vault: Invalid owner"),
                12 => invalid_pubkey(InstructionError::InvalidAccountData, "Reserve vault: Invalid pubkey"),
                13 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: Invalid program id"),
                14 => invalid_program_id(InstructionError::IncorrectProgramId, "Drift program: Invalid program id"),
            }
        )?;

        Ok(())
    }

    #[test]
    fn drift_sync_invalid_inner_accounts() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let spot_market_index = 0;
        setup_drift_state(&mut svm);

        // Initialize Token Mint
        let token_mint_kp = Keypair::new();
        let token_mint = token_mint_kp.pubkey();
        let mint_authority = Keypair::new();
        let oracle_price = 100;

        initialize_mint(
            &mut svm,
            &super_authority,
            &mint_authority.pubkey(),
            None,
            6,
            Some(token_mint_kp),
            &spl_token::ID,
            None,
            None,
        )?;

        let pool_id = 0;

        let spot_market = set_drift_spot_market(
            &mut svm,
            spot_market_index,
            &token_mint,
            oracle_price,
            pool_id,
        );

        setup_drift_spot_market_vault(&mut svm, spot_market_index, &token_mint, &spl_token::ID);

        setup_mock_oracle_account(&mut svm, &spot_market.oracle, oracle_price);

        // Initialize Drift Integration
        let sub_account_id = 0;
        let rate_limit_slope = 1_000_000_000_000;
        let rate_limit_max_outflow = 2_000_000_000_000;
        let permit_liquidation = true;
        let init_ix = create_drift_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &token_mint,
            "Drift Lend",
            IntegrationStatus::Active,
            rate_limit_slope,
            rate_limit_max_outflow,
            permit_liquidation,
            sub_account_id,
            spot_market_index,
            pool_id,
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Initialize a reserve for the token
        let reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &token_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            &spl_token::ID,
        )?;

        // Create the sync instruction
        let sync_ix = create_drift_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pubkey,
            &reserve_keys.pubkey,
            &spot_market.oracle,
            spot_market_index,
            sub_account_id,
        )?;

        // Test invalid accounts for the inner context accounts (remaining_accounts)
        // The remaining_accounts start at index 5 (after controller, controller_authority, payer, integration, reserve)
        // Inner accounts are: spot_market_vault(5), spot_market(6), user(7), drift_program(8)
        test_invalid_accounts!(
            svm,
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            sync_ix,
            {
                5 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift state: Invalid owner"),
                6 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift spot market vault: Invalid owner"),
                6 => invalid_pubkey(InstructionError::Custom(1), "Drift spot market vault: Invalid pubkey"),
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift spot market: Invalid owner"),
                7 => invalid_pubkey(InstructionError::Custom(1), "Drift spot market: Invalid pubkey"),
                8 => invalid_pubkey(InstructionError::Custom(2), "Drift oracle: Did not match SpotMarket oracle"),
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "Drift user: Invalid owner"),
                9 => invalid_pubkey(InstructionError::Custom(1), "Drift user: Invalid pubkey"),
                10 => invalid_program_id(InstructionError::IncorrectProgramId, "Drift program: Invalid program id"),
            }
        )?;

        Ok(())
    }
}
