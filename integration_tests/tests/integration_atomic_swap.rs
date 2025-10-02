mod helpers;
mod subs;

#[cfg(test)]
mod tests {
    use std::i64;

    use crate::{
        assert_contains_controller_cpi_event, helpers::{
            assert::{assert_custom_error, assert_program_error},
            lite_svm_with_programs,
        }, subs::{
            atomic_swap_borrow_repay, atomic_swap_borrow_repay_ixs, derive_controller_authority_pda, derive_permission_pda, fetch_integration_account, fetch_reserve_account, fetch_token_account, get_mint, initialize_ata, initialize_mint, initialize_reserve, manage_controller, mint_tokens, sync_reserve, transfer_tokens, ReserveKeys
        }
    };
    use borsh::BorshDeserialize;
    use litesvm::LiteSVM;
    use solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        generated::types::{
            AccountingAction, AccountingDirection, AccountingEvent, ControllerStatus, IntegrationConfig, IntegrationState, IntegrationStatus, IntegrationUpdateEvent, PermissionStatus, ReserveStatus, SvmAlmControllerEvent
        },
        create_atomic_swap_initialize_integration_instruction,
    };

    use test_case::test_case;

    use crate::subs::{
        initialize_contoller, manage_permission,
        oracle::{derive_oracle_pda, initialize_oracle, set_price_feed},
    };

    struct SwapEnv {
        pub relayer_authority_kp: Keypair,
        pub mint_authority: Keypair,
        pub price_feed: Pubkey,
        pub nonce: Pubkey,
        pub oracle: Pubkey,
        pub pc_token_mint: Pubkey,
        pub coin_token_mint: Pubkey,
        pub controller_pk: Pubkey,
        pub controller_authority: Pubkey,
        pub atomic_swap_integration_pk: Pubkey,
        pub relayer_pc: Pubkey,
        pub relayer_coin: Pubkey,
        pub pc_reserve_pubkey: Pubkey,
        pub pc_reserve_vault: Pubkey,
        pub coin_reserve_pubkey: Pubkey,
        pub coin_reserve_vault: Pubkey,
        pub permission_pda: Pubkey,
    }

    fn setup_integration_env(
        svm: &mut LiteSVM,
        expiry_timestamp: i64,
        coin_token_program: &Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_program: &Pubkey,
        pc_token_transfer_fee: Option<u16>,
        invert_price_feed: bool,
        max_staleness: u64,
        skip_initialize_integration: bool,
    ) -> Result<SwapEnv, Box<dyn std::error::Error>> {
        let relayer_authority_kp = Keypair::new();
        let price_feed = Pubkey::new_unique();
        let nonce = Pubkey::new_unique();
        let coin_token_mint_kp = Keypair::new();
        let coin_token_mint = coin_token_mint_kp.pubkey();
        let pc_token_mint_kp = Keypair::new();
        let pc_token_mint = pc_token_mint_kp.pubkey();
        let mint_authority = Keypair::new();

        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        // Initialize price feed and oracle.
        let update_slot = 1000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(svm, &price_feed, 1_000_000_000_000)?; // $1

        initialize_mint(
            svm,
            &relayer_authority_kp,
            &mint_authority.pubkey(),
            None,
            6,
            Some(coin_token_mint_kp),
            coin_token_program,
            coin_token_transfer_fee,
        )?;
        initialize_mint(
            svm,
            &relayer_authority_kp,
            &mint_authority.pubkey(),
            None,
            6,
            Some(pc_token_mint_kp),
            pc_token_program,
            pc_token_transfer_fee,
        )?;

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let (tx_result, _) = initialize_oracle(
            svm,
            &controller_pk,
            &relayer_authority_kp,
            &nonce,
            &price_feed,
            0,
            &pc_token_mint,
            &coin_token_mint,
        );

        tx_result.map_err(|e| e.err.to_string())?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let _ = manage_permission(
            svm,
            &controller_pk,
            &relayer_authority_kp,          // payer
            &relayer_authority_kp,          // calling authority
            &relayer_authority_kp.pubkey(), // subject authority
            PermissionStatus::Active,
            true,  // can_execute_swap,
            true,  // can_manage_permissions,
            true,  // can_invoke_external_transfer,
            false, // can_reallocate,
            true, // can_freeze,
            false, // can_unfreeze,
            true,  // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        // Setup relayer with funded token accounts.
        initialize_ata(
            svm,
            &relayer_authority_kp,
            &relayer_authority_kp.pubkey(),
            &pc_token_mint,
        )?;
        mint_tokens(
            svm,
            &relayer_authority_kp,
            &mint_authority,
            &pc_token_mint,
            &relayer_authority_kp.pubkey(),
            1_000_000_000_000,
        )?;
        initialize_ata(
            svm,
            &relayer_authority_kp,
            &relayer_authority_kp.pubkey(),
            &coin_token_mint,
        )?;
        mint_tokens(
            svm,
            &relayer_authority_kp,
            &mint_authority,
            &coin_token_mint,
            &relayer_authority_kp.pubkey(),
            1_000_000_000,
        )?;
        let relayer_pc = get_associated_token_address_with_program_id(
            &relayer_authority_kp.pubkey(),
            &pc_token_mint,
            pc_token_program,
        );
        let relayer_coin = get_associated_token_address_with_program_id(
            &relayer_authority_kp.pubkey(),
            &coin_token_mint,
            coin_token_program,
        );

        let ReserveKeys {
            pubkey: pc_reserve_pubkey,
            vault: pc_reserve_vault,
        } = initialize_reserve(
            svm,
            &controller_pk,
            &pc_token_mint,        // mint
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // authority
            ReserveStatus::Active,
            1_000_000_000, // rate_limit_slope
            1_000_000_000, // rate_limit_max_outflow
            pc_token_program,
        )?;

        let ReserveKeys {
            pubkey: coin_reserve_pubkey,
            vault: coin_reserve_vault,
        } = initialize_reserve(
            svm,
            &controller_pk,
            &coin_token_mint,      // mint
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            coin_token_program,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            &pc_token_mint,
            &controller_authority,
            300_000_000,
        )?;
        transfer_tokens(
            svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            &coin_token_mint,
            &controller_authority,
            600_000_000,
        )?;

        // Make sure Reserve accounts start with updated balances. This
        // is not necessary, but is helpful for testing.
        sync_reserve(svm, &controller_pk, &pc_token_mint, &relayer_authority_kp)?;
        sync_reserve(svm, &controller_pk, &coin_token_mint, &relayer_authority_kp)?;

        let permission_pda = derive_permission_pda(&controller_pk, &relayer_authority_kp.pubkey());

        // Initialize an AtomicSwap integration
        let oracle = derive_oracle_pda(&nonce);
        let atomic_swap_integration_pk = if !skip_initialize_integration {
            let rate_limit_slope = 1_000_000;
            let rate_limit_max_outflow = 1_000_000;
            let max_slippage = 123;
            let permit_liquidation = false;
            let input_token = if invert_price_feed {
                coin_token_mint
            } else {
                pc_token_mint
            };
            let output_token = if invert_price_feed {
                pc_token_mint
            } else {
                coin_token_mint
            };
            let init_ix = create_atomic_swap_initialize_integration_instruction(
                &relayer_authority_kp.pubkey(),
                &controller_pk,                 // controller
                &relayer_authority_kp.pubkey(), // authority
                "Pc to Coin swap",
                IntegrationStatus::Active,
                rate_limit_slope,       // rate_limit_slope
                rate_limit_max_outflow, // rate_limit_max_outflow
                permit_liquidation,     // permit_liquidation
                &input_token,
                6,             // input_mint_decimals
                &output_token, // output_token
                6,             // output_mint_decimals
                &oracle,       // oracle
                max_staleness, // max_staleness
                expiry_timestamp,
                max_slippage,      // max_slippage_bps
                invert_price_feed, // oracle_price_inverted
            );
            let integration_pubkey = init_ix.accounts[5].pubkey;
            let tx = Transaction::new_signed_with_payer(
                &[init_ix],
                Some(&relayer_authority_kp.pubkey()),
                &[&relayer_authority_kp],
                svm.latest_blockhash(),
            );
            svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

            integration_pubkey
        } else {
            Pubkey::default()
        };

        Ok(SwapEnv {
            relayer_authority_kp,
            mint_authority,
            price_feed,
            nonce,
            oracle,
            pc_token_mint,
            coin_token_mint,
            controller_pk,
            controller_authority,
            atomic_swap_integration_pk,
            relayer_pc,
            relayer_coin,
            pc_reserve_pubkey,
            pc_reserve_vault,
            coin_reserve_pubkey,
            coin_reserve_vault,
            permission_pda,
        })
    }

    #[test_case( spl_token::ID, spl_token::ID ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID ; "Coin Token2022, PC Token2022")]
    fn init_atomic_swap(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        // note: warped to slot: 1_000_000
        let expiry_timestamp = 1_000_000 + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            None,
            &pc_token_program,
            None,
            false,
            100,
            true,
        )?;
        let clock = svm.get_sysvar::<Clock>();

        let rate_limit_slope = 1_000_000;
        let rate_limit_max_outflow = 1_000_000;
        let max_staleness = 100;
        let max_slippage = 123;
        let permit_liquidation = true;
        let init_ix = create_atomic_swap_initialize_integration_instruction(
            &swap_env.relayer_authority_kp.pubkey(),
            &swap_env.controller_pk,                 // controller
            &swap_env.relayer_authority_kp.pubkey(), // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            rate_limit_slope,          // rate_limit_slope
            rate_limit_max_outflow,    // rate_limit_max_outflow
            permit_liquidation,        // permit_liquidation
            &swap_env.pc_token_mint,   // input_token
            6,                         // input_mint_decimals
            &swap_env.coin_token_mint, // output_token
            6,                         // output_mint_decimals
            &swap_env.oracle,          // oracle
            max_staleness,             // max_staleness
            expiry_timestamp,
            max_slippage, // max_slippage_bps
            false,        // oracle_price_inverted
        );
        let integration_pubkey = init_ix.accounts[5].pubkey;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(tx.clone())
            .map_err(|e| e.err.to_string())?;

        // Check that integration after init.
        let integration = fetch_integration_account(&mut svm, &integration_pubkey)?.unwrap();

        assert_eq!(integration.controller, swap_env.controller_pk);
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

        if let (IntegrationConfig::AtomicSwap(cfg), IntegrationState::AtomicSwap(state)) =
            (&integration.config, integration.clone().state)
        {
            assert_eq!(cfg.input_token, swap_env.pc_token_mint);
            assert_eq!(cfg.output_token, swap_env.coin_token_mint);
            assert_eq!(cfg.oracle, swap_env.oracle);
            assert_eq!(cfg.max_slippage_bps, max_slippage);
            assert_eq!(cfg.max_staleness, max_staleness);
            assert_eq!(cfg.input_mint_decimals, 6);
            assert_eq!(cfg.output_mint_decimals, 6);
            assert_eq!(cfg.expiry_timestamp, expiry_timestamp);

            assert_eq!(state.last_balance_a, 0);
            assert_eq!(state.last_balance_b, 0);
            assert_eq!(state.amount_borrowed, 0);
        } else {
            assert!(false)
        }

        // Assert event is emitted
        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: swap_env.controller_pk,
            integration: integration_pubkey,
            authority: swap_env.relayer_authority_kp.pubkey(),
            old_state: None,
            new_state: Some(integration),
        });
        assert_contains_controller_cpi_event!(
            tx_result, 
            tx.message.account_keys.as_slice(), 
            expected_event
        );

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Coin Token2022, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, Some(100), None ; "Coin Token2022 TransferFee 100 bps, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, Some(100) ; "Coin Token2022, PC Token2022 TransferFee 100 bps")]
    fn atomic_swap_success(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let _integration =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?;

        let vault_a_before = fetch_token_account(&mut svm, &swap_env.pc_reserve_vault);
        let vault_b_before = fetch_token_account(&mut svm, &swap_env.coin_reserve_vault);
        let relayer_a_before = fetch_token_account(&mut svm, &swap_env.relayer_pc);
        let relayer_b_before = fetch_token_account(&mut svm, &swap_env.relayer_coin);
        let reserve_a_before =
            fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();
        let reserve_b_before =
            fetch_reserve_account(&mut svm, &swap_env.coin_reserve_pubkey)?.unwrap();

        let borrow_amount = 100;
        let expected_borrow_amount = if let Some(transfer_fee) = pc_token_transfer_fee {
            borrow_amount - (borrow_amount * u64::from(transfer_fee) / 10_000)
        } else {
            borrow_amount
        };
        let repay_amount = 300;
        let expected_repay_amount = if let Some(transfer_fee) = coin_token_transfer_fee {
            repay_amount - (repay_amount * u64::from(transfer_fee) / 10_000)
        } else {
            repay_amount
        };

        // REserve A outflow: args.amount
        // Reserve A inflow: excess?
        // Reserve B inflow: args.amount // should be repay amount?

        // outflow will be borrow_amount
        // inflow will be expected_repay_amount

        atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            // Use can only spend expected borrow amount as they receive
            // borrow_amount - TransferFees.
            expected_borrow_amount,
        )
        .unwrap();

        let vault_a_after = fetch_token_account(&mut svm, &swap_env.pc_reserve_vault);
        let vault_b_after = fetch_token_account(&mut svm, &swap_env.coin_reserve_vault);
        let relayer_a_after = fetch_token_account(&mut svm, &swap_env.relayer_pc);
        let relayer_b_after = fetch_token_account(&mut svm, &swap_env.relayer_coin);
        let reserve_a_after =
            fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();
        let reserve_b_after =
            fetch_reserve_account(&mut svm, &swap_env.coin_reserve_pubkey)?.unwrap();

        let vault_a_decrease = vault_a_before
            .amount
            .checked_sub(vault_a_after.amount)
            .unwrap();
        let vault_b_increase = vault_b_after
            .amount
            .checked_sub(vault_b_before.amount)
            .unwrap();
        let relayer_b_decrease = relayer_b_before
            .amount
            .checked_sub(relayer_b_after.amount)
            .unwrap();
        let relayer_a_increase = relayer_a_after
            .amount
            .checked_sub(relayer_a_before.amount)
            .unwrap();

        // Check that token balances are changed as expected.
        // relayer_b_decrease should be 0 because tokens are minted and then immediately repaid
        assert_eq!(vault_a_decrease, borrow_amount);
        assert_eq!(relayer_a_increase, 0);
        assert_eq!(vault_b_increase, expected_repay_amount);
        assert_eq!(relayer_b_decrease, 0);

        // Check Reserve state accounted properly.
        let reserve_a_last_balance_delta = reserve_a_before
            .last_balance
            .checked_sub(reserve_a_after.last_balance)
            .expect("overflow");
        let reserve_a_outflow_available_delta = reserve_a_before
            .rate_limit_outflow_amount_available
            .checked_sub(reserve_a_after.rate_limit_outflow_amount_available)
            .expect("overflow");
        let reserve_b_last_balance_delta = reserve_b_after
            .last_balance
            .checked_sub(reserve_b_before.last_balance)
            .expect("overflow");
        let reserve_b_outflow_available_delta = reserve_b_before
            .rate_limit_outflow_amount_available
            .checked_sub(reserve_b_after.rate_limit_outflow_amount_available)
            .expect("overflow");
        assert_eq!(reserve_a_last_balance_delta, borrow_amount);
        assert_eq!(reserve_a_outflow_available_delta, borrow_amount);
        assert_eq!(reserve_b_last_balance_delta, expected_repay_amount);
        // should be unchanged as it started as the max amount
        assert_eq!(reserve_b_outflow_available_delta, 0);

        // Check that integration after swap.
        let integration =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();

        if let (IntegrationConfig::AtomicSwap(cfg), IntegrationState::AtomicSwap(state)) =
            (&integration.config, integration.state)
        {
            assert_eq!(cfg.input_token, swap_env.pc_token_mint);
            assert_eq!(cfg.output_token, swap_env.coin_token_mint);
            assert_eq!(cfg.oracle, swap_env.oracle);
            assert_eq!(cfg.max_slippage_bps, 123);
            assert_eq!(cfg.max_staleness, 100);
            assert_eq!(cfg.input_mint_decimals, 6);
            assert_eq!(cfg.output_mint_decimals, 6);
            assert_eq!(cfg.expiry_timestamp, expiry_timestamp);

            assert_eq!(state.last_balance_a, 0);
            assert_eq!(state.last_balance_b, 0);
            assert_eq!(state.amount_borrowed, 0);
        } else {
            assert!(false)
        }

        // Create a random user
        let random_user = Pubkey::new_unique();
        let random_user_pc_token = initialize_ata(
            &mut svm,
            &swap_env.relayer_authority_kp,
            &random_user,
            &swap_env.pc_token_mint,
        )?;

        let vault_a_before = fetch_token_account(&mut svm, &swap_env.pc_reserve_vault);
        let vault_b_before = fetch_token_account(&mut svm, &swap_env.coin_reserve_vault);
        let relayer_a_before = fetch_token_account(&mut svm, &swap_env.relayer_pc);
        let relayer_b_before = fetch_token_account(&mut svm, &swap_env.relayer_coin);

        let [refresh_ix, borrow_ix, mint_ix, _burn_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            pc_token_program.clone(),
            coin_token_program.clone(),
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            0,
        );

        // Transfer some tokens out of relayer_pc to simulate spending.
        let spent_a = 15;
        let pc_mint = get_mint(&svm, &swap_env.pc_token_mint);
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &pc_token_program,
            &swap_env.relayer_pc,
            &swap_env.pc_token_mint,
            &random_user_pc_token,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            spent_a,
            pc_mint.decimals,
        )?;

        let txn = Transaction::new_signed_with_payer(
            &[refresh_ix, borrow_ix, mint_ix, transfer_ix, repay_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp, &swap_env.mint_authority],
            svm.latest_blockhash(),
        );
        let txn_result = svm.send_transaction(txn.clone());
        txn_result.clone().unwrap();

        // Because the AtomicSwap happened twice, we must double the amount lost from TransferFees.
        let total_transfer_fees_on_borrow = 2 * (borrow_amount - expected_borrow_amount);

        let vault_a_after = fetch_token_account(&mut svm, &swap_env.pc_reserve_vault);
        let vault_b_after = fetch_token_account(&mut svm, &swap_env.coin_reserve_vault);
        let relayer_a_after = fetch_token_account(&mut svm, &swap_env.relayer_pc);
        let relayer_b_after = fetch_token_account(&mut svm, &swap_env.relayer_coin);

        let vault_a_decrease = vault_a_before
            .amount
            .checked_sub(vault_a_after.amount)
            .unwrap();
        let vault_b_increase = vault_b_after
            .amount
            .checked_sub(vault_b_before.amount)
            .unwrap();
        let relayer_b_decrease = relayer_b_before
            .amount
            .checked_sub(relayer_b_after.amount)
            .unwrap();
        let relayer_a_increase = relayer_a_after
            .amount
            .checked_sub(relayer_a_before.amount)
            .unwrap();

        // Check that net change for relayer_a is 0 as excess tokens are repaid.
        assert_eq!(vault_a_decrease, spent_a + total_transfer_fees_on_borrow);
        assert_eq!(relayer_a_increase, 0);
        assert_eq!(vault_b_increase, expected_repay_amount);
        assert_eq!(relayer_b_decrease, 0);

        let final_input_amount = vault_a_decrease;
        // Assert event was emitted
        let expected_debit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: swap_env.controller_pk,
            integration: None,
            reserve: Some(swap_env.pc_reserve_pubkey),
            mint: swap_env.pc_token_mint,
            action: AccountingAction::Swap,
            delta: final_input_amount,
            direction: AccountingDirection::Debit,
        });
        assert_contains_controller_cpi_event!(
            txn_result.clone().unwrap(), 
            txn.message.account_keys.as_slice(), 
            expected_debit_event
        );

        let balance_b_delta = vault_b_after.amount - vault_b_before.amount;
        let expected_credit_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: swap_env.controller_pk,
            integration: None,
            reserve: Some(swap_env.coin_reserve_pubkey),
            mint: swap_env.coin_token_mint,
            action: AccountingAction::Swap,
            delta: balance_b_delta,
            direction: AccountingDirection::Credit,
        });
        assert_contains_controller_cpi_event!(
            txn_result.unwrap(), 
            txn.message.account_keys.as_slice(), 
            expected_credit_event
        );

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None, false ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None, false ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None, false ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None, false ; "Coin Token2022, PC Token2022")]
    #[test_case( spl_token::ID, spl_token::ID, None, None, true ; "Inverted Oracle Price")]
    fn atomic_swap_slippage_checks(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
        invert_price_feed: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            invert_price_feed,
            100,
            false,
        )?;

        let _integration =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?;

        let borrow_amount = 100;
        let repay_amount = 300; // At rate of 3.0

        if invert_price_feed {
            // rate of .2, so inverted is 5
            set_price_feed(&mut svm, &swap_env.price_feed, 200_000_000_000_000_000)?;
        } else {
            // rate of 3.3
            set_price_feed(&mut svm, &swap_env.price_feed, 3_300_000_000_000_000_000)?;
        }

        let (mint_a, mint_b, recipient_a, recipient_b) = if invert_price_feed {
            (
                swap_env.coin_token_mint,
                swap_env.pc_token_mint,
                swap_env.relayer_coin,
                swap_env.relayer_pc,
            )
        } else {
            (
                swap_env.pc_token_mint,
                swap_env.coin_token_mint,
                swap_env.relayer_pc,
                swap_env.relayer_coin,
            )
        };

        // Should fail when slippage is exceeded (since min price of 3.3*(1-0.0123) < 3.0)
        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            mint_a,
            mint_b,
            swap_env.oracle,
            swap_env.price_feed,
            recipient_a, // payer_account_a
            recipient_b, // payer_account_b
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );
        assert_custom_error(&res, 4, SvmAlmControllerErrors::SlippageExceeded);

        // Swap Price (after excess repaid) = 300/50 = 6
        let spent_a = 50;
        let borrow_amount = 100;
        let repay_amount = 300;

        if invert_price_feed {
            // rate of .1, so inverted is 10
            set_price_feed(&mut svm, &swap_env.price_feed, 100_000_000_000_000_000)?;
        } else {
            // rate of 6.1
            set_price_feed(&mut svm, &swap_env.price_feed, 6_100_000_000_000_000_000)?;
        }

        // Should fail when slippage is exceeded (since min price of 6.1*(1-0.0123) < 6.0)

        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            mint_a,
            mint_b,
            swap_env.oracle,
            swap_env.price_feed,
            recipient_a, // payer_account_a
            recipient_b, // payer_account_b
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            spent_a,
        );

        assert_custom_error(&res, 4, SvmAlmControllerErrors::SlippageExceeded);

        let (mint_a_token_program, mint_b_token_program) = if mint_b == swap_env.pc_token_mint {
            (coin_token_program, pc_token_program)
        } else {
            (pc_token_program, coin_token_program)
        };
        let [refresh_ix, borrow_ix, _mint_ix, _burn_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            mint_a,
            mint_b,
            swap_env.oracle,
            swap_env.price_feed,
            recipient_a, // payer_account_a
            recipient_b, // payer_account_b
            mint_a_token_program,
            mint_b_token_program,
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );

        // Burn tokens to trigger underflow when calculating Tokne B diff during Repay.
        let burn_token_b_ix = spl_token_2022::instruction::burn_checked(
            &mint_b_token_program,
            &recipient_b,
            &mint_b,
            &swap_env.relayer_authority_kp.pubkey(),
            &[],
            10,
            6,
        )
        .unwrap();

        let txn = Transaction::new_signed_with_payer(
            &[
                refresh_ix.clone(),
                borrow_ix.clone(),
                burn_token_b_ix,
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_eq!(
            res.err().unwrap().err,
            TransactionError::InstructionError(3, InstructionError::ProgramFailedToComplete)
        );

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Coin Token2022, PC Token2022")]
    fn atomic_swap_fails_after_expiry(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let borrow_amount = 100;
        let repay_amount = 300;

        // Do one round of swap first
        atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        )
        .unwrap();

        let mut clock = svm.get_sysvar::<Clock>();
        clock.unix_timestamp = expiry_timestamp + 1;
        svm.set_sysvar::<Clock>(&clock);

        // Expect failure after expiry
        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            borrow_amount + 10,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );

        assert_custom_error(&res, 1, SvmAlmControllerErrors::IntegrationHasExpired);
        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Coin Token2022, PC Token2022")]
    fn atomic_swap_fails_with_invalid_token_amounts(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let repay_amount = 300;
        let borrow_amount = 100;

        // Expect failure when repay amount is 0.
        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            borrow_amount,
            0,
            &swap_env.mint_authority,
            borrow_amount,
        );
        assert_program_error(&res, 4, InstructionError::InsufficientFunds);

        // Expect failure when borrowing more than balance.
        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            300_000_001,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );

        assert_program_error(&res, 1, InstructionError::InsufficientFunds);

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Coin Token2022, PC Token2022")]
    fn atomic_swap_vault_balance_check(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let borrow_amount = 100;
        let repay_amount = 300;
        let mid_tx_transfer_amount = 222;

        let [refresh_ix, borrow_ix, mint_ix, _burn_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            pc_token_program.clone(),
            coin_token_program.clone(),
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );

        // Transfer some to vault_a
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &pc_token_program,
            &swap_env.relayer_pc,
            &swap_env.pc_token_mint,
            &swap_env.pc_reserve_vault,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            mid_tx_transfer_amount,
            6,
        )?;

        // Expect failure when vault balances are modified btw borrow and repay.
        let txn = Transaction::new_signed_with_payer(
            &[
                refresh_ix.clone(),
                borrow_ix.clone(),
                mint_ix.clone(),
                transfer_ix,
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp, &swap_env.mint_authority],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 4, SvmAlmControllerErrors::InvalidSwapState);

        // Transfer some to vault_b
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &coin_token_program,
            &swap_env.relayer_coin,
            &swap_env.coin_token_mint,
            &swap_env.coin_reserve_vault,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            mid_tx_transfer_amount,
            6,
        )?;

        // Expect failure when vault balances are modified btw borrow and repay.
        let txn = Transaction::new_signed_with_payer(
            &[
                refresh_ix.clone(),
                borrow_ix.clone(),
                mint_ix.clone(),
                transfer_ix,
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp, &swap_env.mint_authority],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 4, SvmAlmControllerErrors::InvalidSwapState);

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Coin Token2022, PC Token2022")]
    fn atomic_swap_ix_ordering_checks(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let borrow_amount = 100;
        let repay_amount = 300;

        let [refresh_ix, borrow_ix, mint_ix, _burn_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            pc_token_program.clone(),
            coin_token_program.clone(),
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );

        // Expect failure when borrowing w/o repay.
        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix.clone(), refresh_ix.clone()],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when repay is not the last ix.
        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix.clone(), repay_ix.clone(), refresh_ix.clone()],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when repay w/o borrowing first.
        let txn = Transaction::new_signed_with_payer(
            &[repay_ix.clone(), borrow_ix.clone(), refresh_ix.clone()],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 0, SvmAlmControllerErrors::SwapNotStarted);

        // Expect failure when borrowing multiple times
        let txn = Transaction::new_signed_with_payer(
            &[
                borrow_ix.clone(),
                borrow_ix.clone(),
                refresh_ix.clone(),
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when repaying multiple times
        let txn = Transaction::new_signed_with_payer(
            &[
                borrow_ix.clone(),
                mint_ix.clone(),
                repay_ix.clone(),
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp, &swap_env.mint_authority],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when mutliple borrow/repays in one TX
        let txn = Transaction::new_signed_with_payer(
            &[
                borrow_ix.clone(),
                mint_ix.clone(),
                repay_ix.clone(),
                borrow_ix.clone(),
                mint_ix.clone(),
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp, &swap_env.mint_authority],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 0, SvmAlmControllerErrors::InvalidInstructions);

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    fn atomic_swap_oracle_checks(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let borrow_amount = 100;
        let repay_amount = 300;

        let [_refresh_ix, borrow_ix, mint_ix, _burn_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            pc_token_program.clone(),
            coin_token_program.clone(),
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );

        let clock = svm.get_sysvar::<Clock>();
        svm.warp_to_slot(clock.slot + 2000);

        // Expect failure when oracle has expired.
        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix.clone(), mint_ix.clone(), repay_ix.clone()],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp, &swap_env.mint_authority],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 2, SvmAlmControllerErrors::StaleOraclePrice);

        // Should error when input/output mint do not match the Oracle mint

        let oracle = derive_oracle_pda(&swap_env.nonce);
        let mint_1_kp = Keypair::new();
        let mint_1_pubkey = mint_1_kp.pubkey();
        let mint_2_kp = Keypair::new();
        let mint_2_pubkey = mint_2_kp.pubkey();
        initialize_mint(
            &mut svm,
            &swap_env.relayer_authority_kp,
            &swap_env.mint_authority.pubkey(),
            None,
            6,
            Some(mint_1_kp),
            &spl_token::ID,
            None,
        )?;
        initialize_mint(
            &mut svm,
            &swap_env.relayer_authority_kp,
            &swap_env.mint_authority.pubkey(),
            None,
            6,
            Some(mint_2_kp),
            &spl_token::ID,
            None,
        )?;

        let init_ix = create_atomic_swap_initialize_integration_instruction(
            &swap_env.relayer_authority_kp.pubkey(),
            &swap_env.controller_pk,                 // controller
            &swap_env.relayer_authority_kp.pubkey(), // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000, // rate_limit_slope
            1_000_000, // rate_limit_max_outflow
            false,     // permit_liquidation
            &mint_1_pubkey,
            6,              // input_mint_decimals
            &mint_2_pubkey, // output_token
            6,              // output_mint_decimals
            &oracle,        // oracle
            123,            // max_staleness
            expiry_timestamp,
            100,   // max_slippage_bps
            false, // oracle_price_inverted
        );
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        ));

        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(SvmAlmControllerErrors::InvalidOracleForMints as u32)
            )
        );

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Coin Token2022, PC Token2022")]
    fn atomic_swap_rate_limit_valid_state(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let borrow_amount = 5_00_000;
        let repay_amount = 30_000_000;

        let integration_pre =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();
        let reserve_pc_pre = fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();

        atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        )
        .unwrap();

        let integration_post =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();
        let reserve_pc_post =
            fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();
        let reserve_coin_post =
            fetch_reserve_account(&mut svm, &swap_env.coin_reserve_pubkey)?.unwrap();

        // Expect outflow by borrow amount
        assert_eq!(
            integration_pre.rate_limit_outflow_amount_available
                - integration_post.rate_limit_outflow_amount_available,
            borrow_amount
        );

        // Expect outflow by borrow amount
        assert_eq!(
            reserve_pc_pre.rate_limit_outflow_amount_available
                - reserve_pc_post.rate_limit_outflow_amount_available,
            borrow_amount
        );

        // Expect no change since its alr at max.
        assert_eq!(
            reserve_coin_post.rate_limit_outflow_amount_available,
            reserve_coin_post.rate_limit_max_outflow
        );

        // Create a random user
        let random_user = Pubkey::new_unique();
        let random_user_pc_token = initialize_ata(
            &mut svm,
            &swap_env.relayer_authority_kp,
            &random_user,
            &swap_env.pc_token_mint,
        )?;

        // Swap with repaying of token_a
        let borrow_amount = 100;
        let repay_amount = 300;

        let [refresh_ix, borrow_ix, mint_ix, _burn_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            pc_token_program.clone(),
            coin_token_program.clone(),
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            0,
        );

        // Transfer some tokens out of relayer_pc to simulate spending.
        let spent_a = 15;
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &pc_token_program,
            &swap_env.relayer_pc,
            &swap_env.pc_token_mint,
            &random_user_pc_token,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            spent_a,
            6,
        )?;

        let txn = Transaction::new_signed_with_payer(
            &[refresh_ix, borrow_ix, mint_ix, transfer_ix, repay_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp, &swap_env.mint_authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(txn).unwrap();

        let integration_post2 =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();
        let reserve_pc_post2 =
            fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();
        let reserve_coin_post2 =
            fetch_reserve_account(&mut svm, &swap_env.coin_reserve_pubkey)?.unwrap();

        // Expect outflow by spent amount
        assert_eq!(
            integration_post.rate_limit_outflow_amount_available
                - integration_post2.rate_limit_outflow_amount_available,
            spent_a
        );

        // Expect outflow by spent amount
        assert_eq!(
            reserve_pc_post.rate_limit_outflow_amount_available
                - reserve_pc_post2.rate_limit_outflow_amount_available,
            spent_a
        );

        // Expect no change since its alr at max.
        assert_eq!(
            reserve_coin_post.rate_limit_outflow_amount_available,
            reserve_coin_post.rate_limit_max_outflow
        );

        // Initialize a different AtomicSwap integration with coin_token_mint as input.
        let oracle_2_nonce = Pubkey::new_unique();
        let oracle_2 = derive_oracle_pda(&oracle_2_nonce);
        let price_feed_2 = Pubkey::new_unique();
        set_price_feed(&mut svm, &price_feed_2, 1_000_000_000_000)?; // $1
        let (tx_result, _) = initialize_oracle(
            &mut svm,
            &swap_env.controller_pk,
            &swap_env.relayer_authority_kp,
            &oracle_2_nonce,
            &price_feed_2,
            0,
            &swap_env.coin_token_mint,
            &swap_env.pc_token_mint,
        );

        tx_result.map_err(|e| e.err.to_string())?;
        let init_ix = create_atomic_swap_initialize_integration_instruction(
            &swap_env.relayer_authority_kp.pubkey(),
            &swap_env.controller_pk,                 // controller
            &swap_env.relayer_authority_kp.pubkey(), // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000, // rate_limit_slope
            1_000_000, // rate_limit_max_outflow
            false,     // permit_liquidation
            &swap_env.coin_token_mint,
            6,                       // input_mint_decimals
            &swap_env.pc_token_mint, // output_token
            6,                       // output_mint_decimals
            &oracle_2,               // oracle
            123,                     // max_staleness
            expiry_timestamp,
            100,   // max_slippage_bps
            false, // oracle_price_inverted
        );
        let integration_pk2 = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        let borrow_amount = 100;
        let repay_amount = 300;

        let integration2_pre = fetch_integration_account(&mut svm, &integration_pk2)?.unwrap();

        let _res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            integration_pk2,
            swap_env.coin_token_mint,
            swap_env.pc_token_mint,
            oracle_2,
            price_feed_2,
            swap_env.relayer_coin, // payer_account_a
            swap_env.relayer_pc,   // payer_account_b
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        )
        .unwrap();

        let integration2_post = fetch_integration_account(&mut svm, &integration_pk2)?.unwrap();
        let reserve_pc_post3 =
            fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();
        let reserve_coin_post3 =
            fetch_reserve_account(&mut svm, &swap_env.coin_reserve_pubkey)?.unwrap();

        // Expect outflow by borrow amount
        assert_eq!(
            integration2_pre.rate_limit_outflow_amount_available
                - integration2_post.rate_limit_outflow_amount_available,
            borrow_amount
        );

        // Expect outflow by borrow amount
        assert_eq!(
            reserve_coin_post2.rate_limit_outflow_amount_available
                - reserve_coin_post3.rate_limit_outflow_amount_available,
            borrow_amount
        );

        // Expect outflow by repay amount
        assert_eq!(
            reserve_pc_post3.rate_limit_outflow_amount_available
                - reserve_pc_post2.rate_limit_outflow_amount_available,
            repay_amount
        );

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, None, None ; "Coin Token, PC Token")]
    #[test_case( spl_token::ID, spl_token_2022::ID, None, None ; "Coin Token, PC Token2022")]
    #[test_case( spl_token_2022::ID, spl_token::ID, None, None ; "Coin Token2022, PC Token")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, None, None ; "Coin Token2022, PC Token2022")]
    fn atomic_swap_rate_limit_violation(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        coin_token_transfer_fee: Option<u16>,
        pc_token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            coin_token_transfer_fee,
            &pc_token_program,
            pc_token_transfer_fee,
            false,
            100,
            false,
        )?;

        let repay_amount = 30_000_000;

        let integration_pre =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();
        let reserve_pc_pre = fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();

        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            integration_pre.rate_limit_max_outflow + 1,
            repay_amount,
            &swap_env.mint_authority,
            0,
        );
        assert_custom_error(&res, 1, SvmAlmControllerErrors::RateLimited);

        // Initialize a different AtomicSwap integration with higher rate limit than reserve.
        let init_ix = svm_alm_controller_client::create_atomic_swap_initialize_integration_instruction(
            &swap_env.relayer_authority_kp.pubkey(),
            &swap_env.controller_pk,                 // controller
            &swap_env.relayer_authority_kp.pubkey(), // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000_000,                             // rate_limit_slope
            reserve_pc_pre.rate_limit_max_outflow * 2, // rate_limit_max_outflow
            false,                                     // permit_liquidation
            &swap_env.pc_token_mint,
            6, // input_mint_decimals
            &swap_env.coin_token_mint,
            6,                // output_mint_decimals
            &swap_env.oracle, // oracle
            123,              // max_staleness
            expiry_timestamp,
            100,   // max_slippage_bps
            false, // oracle_price_inverted
        );
        let integration_pk2 = init_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &swap_env.relayer_authority_kp,
            &swap_env.relayer_authority_kp,
            &swap_env.pc_token_mint,
            &swap_env.controller_authority,
            reserve_pc_pre.rate_limit_max_outflow * 2,
        )?;

        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            integration_pk2,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            reserve_pc_pre.rate_limit_max_outflow + 1,
            repay_amount,
            &swap_env.mint_authority,
            0,
        );
        assert_custom_error(&res, 1, SvmAlmControllerErrors::RateLimited);

        Ok(())
    }

    #[test]
    fn test_atomic_swap_init_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let relayer_authority_kp = Keypair::new();
        let price_feed = Pubkey::new_unique();
        let nonce = Pubkey::new_unique();
        let coin_token_mint_kp = Keypair::new();
        let coin_token_mint = coin_token_mint_kp.pubkey();
        let pc_token_mint_kp = Keypair::new();
        let pc_token_mint = pc_token_mint_kp.pubkey();
        let mint_authority = Keypair::new();

        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        // Initialize price feed and oracle.
        let update_slot = 1000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(&mut svm, &price_feed, 1_000_000_000_000)?; // $1

        initialize_mint(
            &mut svm,
            &relayer_authority_kp,
            &mint_authority.pubkey(),
            None,
            6,
            Some(coin_token_mint_kp),
            &spl_token::ID,
            None,
        )?;
        initialize_mint(
            &mut svm,
            &relayer_authority_kp,
            &mint_authority.pubkey(),
            None,
            6,
            Some(pc_token_mint_kp),
            &spl_token::ID,
            None,
        )?;

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        
        initialize_oracle(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp,
            &nonce,
            &price_feed,
            0,
            &pc_token_mint,
            &coin_token_mint,
        );

        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp,          // payer
            &relayer_authority_kp,          // calling authority
            &relayer_authority_kp.pubkey(), // subject authority
            PermissionStatus::Active,
            true,  // can_execute_swap,
            true,  // can_manage_permissions,
            true,  // can_invoke_external_transfer,
            false, // can_reallocate,
            true, // can_freeze,
            false, // can_unfreeze,
            true,  // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
            false, // can_liquidate
        )?;

        manage_controller(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Try to initialize an AtomicSwap integration
        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let oracle = derive_oracle_pda(&nonce);
        let init_ix = create_atomic_swap_initialize_integration_instruction(
            &relayer_authority_kp.pubkey(),
            &controller_pk,                 // controller
            &relayer_authority_kp.pubkey(), // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000, // rate_limit_slope
            1_000_000, // rate_limit_max_outflow
            false,     // permit_liquidation
            &pc_token_mint,   // input_token
            6,                // input_mint_decimals
            &coin_token_mint, // output_token
            6,                // output_mint_decimals
            &oracle,          // oracle
            100,              // max_staleness
            expiry_timestamp,
            123,   // max_slippage_bps
            false, // oracle_price_inverted
        );
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&relayer_authority_kp.pubkey()),
            &[&relayer_authority_kp],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test]
    fn test_atomic_swap_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &spl_token::ID,
            None,
            &spl_token::ID,
            None,
            false,
            100,
            false,
        )?;

        manage_controller(
            &mut svm,
            &swap_env.controller_pk,
            &swap_env.relayer_authority_kp, // payer
            &swap_env.relayer_authority_kp, // calling authority
            ControllerStatus::Frozen,
        )?;

        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,   // payer_account_a
            swap_env.relayer_coin, // payer_account_b
            100,
            300,
            &swap_env.mint_authority,
            0,
        );

        assert_custom_error(
            &res,
            1,
            SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction,
        );

        Ok(())
    }

    #[test_case( spl_token::ID, spl_token::ID, 1; "Coin Token, PC Token, max_staleness 1")]
    #[test_case( spl_token::ID, spl_token::ID, 100000; "Coin Token, PC Token, max_staleness 100000")]
    #[test_case( spl_token::ID, spl_token_2022::ID, 1; "Coin Token, PC Token2022, max_staleness 1")]
    #[test_case( spl_token::ID, spl_token_2022::ID, 100000; "Coin Token, PC Token2022, max_staleness 100000")]
    #[test_case( spl_token_2022::ID, spl_token::ID, 1; "Coin Token2022, PC Token, max_staleness 1")]
    #[test_case( spl_token_2022::ID, spl_token::ID, 100000; "Coin Token2022, PC Token, max_staleness 100000")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, 1; "Coin Token2022, PC Token2022, max_staleness 1")]
    #[test_case( spl_token_2022::ID, spl_token_2022::ID, 100000; "Coin Token2022, PC Token2022, max_staleness 100000")]
    fn atomic_swap_oracle_staleness_checks(
        coin_token_program: Pubkey,
        pc_token_program: Pubkey,
        max_staleness: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(
            &mut svm,
            expiry_timestamp,
            &coin_token_program,
            None,
            &pc_token_program,
            None,
            false,
            max_staleness,
            false,
        )?;

        let borrow_amount = 100;
        let repay_amount = 300;

        // Advance clock slot by max_stallenes + 1, staleness check in atomic_swap_repay should throw error
        let mut clock = svm.get_sysvar::<Clock>();
        clock.slot += max_staleness + 1;
        svm.set_sysvar(&clock);

        let res = atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            swap_env.atomic_swap_integration_pk,
            swap_env.pc_token_mint,
            swap_env.coin_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_pc,
            swap_env.relayer_coin,
            borrow_amount,
            repay_amount,
            &swap_env.mint_authority,
            borrow_amount,
        );

        // Assert it always errors since oracle is stale
        // Repay instruction is at index 4
        assert_custom_error(&res, 4, SvmAlmControllerErrors::StaleOraclePrice);

        let integration =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();

        // Check that integration staleness config is correct and unchanged
        if let IntegrationConfig::AtomicSwap(cfg) = &integration.config {
            assert_eq!(cfg.max_staleness, max_staleness);
        } else {
            assert!(false)
        }

        Ok(())
    }
}
