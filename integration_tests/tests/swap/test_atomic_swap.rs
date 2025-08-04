#[cfg(test)]
mod tests {
    use crate::{
        helpers::{
            assert::{assert_custom_error, assert_program_error},
            spl::setup_token_account,
        },
        subs::{
            atomic_swap_borrow_repay, atomic_swap_borrow_repay_ixs,
            derive_controller_authority_pda, derive_permission_pda, fetch_integration_account,
            fetch_reserve_account, fetch_token_account, initialize_ata, initialize_reserve,
            transfer_tokens, ReserveKeys,
        },
    };
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
    use svm_alm_controller_client::generated::types::{
        AtomicSwapConfig, ControllerStatus, FeedArgs, InitializeArgs, IntegrationConfig,
        IntegrationState, IntegrationStatus, PermissionStatus, ReserveStatus,
    };

    use crate::{
        helpers::spl::setup_token_mint,
        subs::{
            initialize_contoller, initialize_integration, manage_permission,
            oracle::{derive_oracle_pda, initalize_oracle, set_price_feed},
        },
    };

    fn lite_svm_with_programs() -> LiteSVM {
        let mut svm = LiteSVM::new();

        // Add the CONTROLLER program
        let controller_program_bytes =
            include_bytes!("../../../target/deploy/svm_alm_controller.so");
        svm.add_program(
            svm_alm_controller_client::SVM_ALM_CONTROLLER_ID,
            controller_program_bytes,
        );

        svm
    }

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
    ) -> Result<SwapEnv, Box<dyn std::error::Error>> {
        let relayer_authority_kp = Keypair::new();
        let price_feed = Pubkey::new_unique();
        let nonce = Pubkey::new_unique();
        let coin_token_mint = Pubkey::new_unique();
        let pc_token_mint = Pubkey::new_unique();
        let mint_authority = Keypair::new();

        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        // Initialize price feed and oracle.
        let update_slot = 1000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(svm, &price_feed, 1_000_000_000_000)?; // $1
        initalize_oracle(svm, &relayer_authority_kp, &nonce, &price_feed, 0, false)?;

        setup_token_mint(svm, &coin_token_mint, 6, &mint_authority.pubkey());
        setup_token_mint(svm, &pc_token_mint, 6, &mint_authority.pubkey());

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let _ = manage_permission(
            svm,
            &controller_pk,
            &relayer_authority_kp,          // payer
            &relayer_authority_kp,          // calling authority
            &relayer_authority_kp.pubkey(), // subject authority
            PermissionStatus::Active,
            true,  // can_execute_swap,
            true, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            true,  // can_manage_integrations
            false,  // can_suspend_permissions
        )?;

        let oracle = derive_oracle_pda(&nonce);

        // Setup relayer with funded token accounts.
        let relayer_pc = get_associated_token_address_with_program_id(
            &relayer_authority_kp.pubkey(),
            &pc_token_mint,
            &pinocchio_token::ID.into(),
        );
        setup_token_account(
            svm,
            &relayer_pc,
            &pc_token_mint,
            &relayer_authority_kp.pubkey(),
            1_000_000_000_000,
            &pinocchio_token::ID.into(),
            None,
        );
        let relayer_coin = get_associated_token_address_with_program_id(
            &relayer_authority_kp.pubkey(),
            &coin_token_mint,
            &pinocchio_token::ID.into(),
        );
        setup_token_account(
            svm,
            &relayer_coin,
            &coin_token_mint,
            &relayer_authority_kp.pubkey(),
            1000_000_000,
            &pinocchio_token::ID.into(),
            None,
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

        let permission_pda = derive_permission_pda(&controller_pk, &relayer_authority_kp.pubkey());

        // Initialize an AtomicSwap integration
        let oracle = derive_oracle_pda(&nonce);
        let atomic_swap_integration_pk = initialize_integration(
            svm,
            &controller_pk,
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000, // rate_limit_slope
            1_000_000, // rate_limit_max_outflow
            &IntegrationConfig::AtomicSwap(AtomicSwapConfig {
                input_token: pc_token_mint,
                output_token: coin_token_mint,
                oracle,
                max_slippage_bps: 123,
                max_staleness: 100,
                input_mint_decimals: 6,
                output_mint_decimals: 6,
                expiry_timestamp,
                padding: [0u8; 108],
            }),
            &InitializeArgs::AtomicSwap {
                max_slippage_bps: 123,
                max_staleness: 100,
                expiry_timestamp,
            },
        )?;

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

    #[test_log::test]
    fn init_atomic_swap() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        // Check that integration after init.
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

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_success() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let _integration =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?;

        let vault_a_before = fetch_token_account(&mut svm, &swap_env.pc_reserve_vault);
        let vault_b_before = fetch_token_account(&mut svm, &swap_env.coin_reserve_vault);
        let relayer_a_before = fetch_token_account(&mut svm, &swap_env.relayer_pc);
        let relayer_b_before = fetch_token_account(&mut svm, &swap_env.relayer_coin);

        let repay_excess_token_a = false;
        let borrow_amount = 100;
        let repay_amount = 300;

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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        )
        .unwrap();

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

        // Check that token balances are changed as expected.
        assert_eq!(vault_a_decrease, borrow_amount);
        assert_eq!(relayer_a_increase, borrow_amount);
        assert_eq!(vault_b_increase, repay_amount);
        assert_eq!(relayer_b_decrease, repay_amount);

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

        // Swap with repaying of token_a
        let repay_excess_token_a = true;
        let borrow_amount = 100;
        let repay_amount = 300;

        let vault_a_before = fetch_token_account(&mut svm, &swap_env.pc_reserve_vault);
        let vault_b_before = fetch_token_account(&mut svm, &swap_env.coin_reserve_vault);
        let relayer_a_before = fetch_token_account(&mut svm, &swap_env.relayer_pc);
        let relayer_b_before = fetch_token_account(&mut svm, &swap_env.relayer_coin);

        let [borrow_ix, refresh_ix, mint_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        // Transfer some tokens out of relayer_pc to simulate spending.
        let spent_a = 15;
        let transfer_ix = spl_token::instruction::transfer(
            &spl_token::id(),
            &swap_env.relayer_pc,
            &random_user_pc_token,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            spent_a,
        )?;

        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix, refresh_ix, mint_ix, transfer_ix, repay_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        svm.send_transaction(txn).unwrap();

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
        assert_eq!(vault_a_decrease, spent_a);
        assert_eq!(relayer_a_increase, 0);
        assert_eq!(vault_b_increase, repay_amount);
        assert_eq!(relayer_b_decrease, repay_amount);

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_slippage_checks() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let _integration =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?;

        let repay_excess_token_a = false;
        let borrow_amount = 100;
        let repay_amount = 300; // At rate of 3.0

        set_price_feed(&mut svm, &swap_env.price_feed, 3_300_000_000_000_000_000)?; // rate of 3.3

        // Should fail when slippage is exceeded (since min price of 3.3*(1-0.0123) < 3.0)
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );
        assert_custom_error(&res, 2, SvmAlmControllerErrors::SlippageExceeded);

        // Swap Price (after excess repaid) = 300/50 = 6
        let repay_excess_token_a = true;
        let spent_a = 50;
        let borrow_amount = 100;
        let repay_amount = 300;

        set_price_feed(&mut svm, &swap_env.price_feed, 6_100_000_000_000_000_000)?; // rate of 6.1

        // Should fail when slippage is exceeded (since min price of 6.1*(1-0.0123) < 6.0)

        let [borrow_ix, refresh_ix, mint_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        // Create a random user
        let random_user = Pubkey::new_unique();
        let random_user_pc_token = initialize_ata(
            &mut svm,
            &swap_env.relayer_authority_kp,
            &random_user,
            &swap_env.pc_token_mint,
        )?;

        // Transfer tokens out of relayer_pc to simulate spending.
        let transfer_ix = spl_token::instruction::transfer(
            &spl_token::id(),
            &swap_env.relayer_pc,
            &random_user_pc_token,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            spent_a,
        )?;

        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix, refresh_ix, mint_ix, transfer_ix, repay_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 3, SvmAlmControllerErrors::SlippageExceeded);

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_fails_after_expiry() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let repay_excess_token_a = false;
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount + 10,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        assert_custom_error(&res, 0, SvmAlmControllerErrors::IntegrationHasExpired);
        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_fails_with_invalid_token_amounts() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let repay_excess_token_a = false;
        let borrow_amount = 0;
        let repay_amount = 300;

        // Expect failure when borrow amount is 0.
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );
        assert_program_error(&res, 0, InstructionError::InvalidArgument);

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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            0,
            swap_env.mint_authority.pubkey(),
        );
        assert_program_error(&res, 2, InstructionError::InvalidArgument);

        // Expect failure when repay amount is more than payer balance.
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            1000_000_001,
            swap_env.mint_authority.pubkey(),
        );
        assert_program_error(&res, 2, InstructionError::InsufficientFunds);

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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            300_000_001,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        assert_program_error(&res, 0, InstructionError::InsufficientFunds);

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_vault_balance_check() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let repay_excess_token_a = false;
        let borrow_amount = 100;
        let repay_amount = 300;

        let [borrow_ix, refresh_ix, mint_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        // Transfer some to vault_a
        let transfer_ix = spl_token::instruction::transfer(
            &spl_token::id(),
            &swap_env.relayer_pc,
            &swap_env.pc_reserve_vault,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            222,
        )?;

        // Expect failure when vault balances are modified btw borrow and repay.
        let txn = Transaction::new_signed_with_payer(
            &[
                borrow_ix.clone(),
                refresh_ix.clone(),
                mint_ix.clone(),
                transfer_ix,
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 3, SvmAlmControllerErrors::InvalidSwapState);

        // Transfer some to vault_b
        let transfer_ix = spl_token::instruction::transfer(
            &spl_token::id(),
            &swap_env.relayer_coin,
            &swap_env.coin_reserve_vault,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            222,
        )?;

        // Expect failure when vault balances are modified btw borrow and repay.
        let txn = Transaction::new_signed_with_payer(
            &[
                borrow_ix.clone(),
                refresh_ix.clone(),
                mint_ix.clone(),
                transfer_ix,
                repay_ix.clone(),
            ],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        println!("{:?}", res);
        assert_custom_error(&res, 3, SvmAlmControllerErrors::InvalidSwapState);

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_ix_ordering_checks() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let repay_excess_token_a = false;
        let borrow_amount = 100;
        let repay_amount = 300;

        let [borrow_ix, refresh_ix, mint_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        // Expect failure when borrowing w/o repay.
        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix.clone(), refresh_ix.clone()],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        println!("LOGS {:?}", res.clone().err().unwrap().meta.logs);
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
        assert_custom_error(&res, 1, SvmAlmControllerErrors::SwapHasStarted);

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_oracle_checks() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let repay_excess_token_a = false;
        let borrow_amount = 100;
        let repay_amount = 300;

        let [borrow_ix, refresh_ix, mint_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        let clock = svm.get_sysvar::<Clock>();
        svm.warp_to_slot(clock.slot + 2000);

        // Expect failure when oracle has expired.
        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix.clone(), mint_ix.clone(), repay_ix.clone()],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
            svm.latest_blockhash(),
        );
        let res = svm.send_transaction(txn);
        assert_custom_error(&res, 1, SvmAlmControllerErrors::StaleOraclePrice);

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_rate_limit_valid_state() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let repay_excess_token_a = false;
        let borrow_amount = 5_00_000;
        let repay_amount = 30_000_000;

        let integration_pre =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();
        let reserve_pc_pre = fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();
        let reserve_coin_pre =
            fetch_reserve_account(&mut svm, &swap_env.coin_reserve_pubkey)?.unwrap();

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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
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
        let repay_excess_token_a = true;
        let borrow_amount = 100;
        let repay_amount = 300;

        let [borrow_ix, refresh_ix, mint_ix, repay_ix] = atomic_swap_borrow_repay_ixs(
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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );

        // Transfer some tokens out of relayer_pc to simulate spending.
        let spent_a = 15;
        let transfer_ix = spl_token::instruction::transfer(
            &spl_token::id(),
            &swap_env.relayer_pc,
            &random_user_pc_token,
            &swap_env.relayer_authority_kp.pubkey(),
            &[&swap_env.relayer_authority_kp.pubkey()],
            spent_a,
        )?;

        let txn = Transaction::new_signed_with_payer(
            &[borrow_ix, refresh_ix, mint_ix, transfer_ix, repay_ix],
            Some(&swap_env.relayer_authority_kp.pubkey()),
            &[&swap_env.relayer_authority_kp],
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
        let integration_pk2 = initialize_integration(
            &mut svm,
            &swap_env.controller_pk,
            &swap_env.relayer_authority_kp, // payer
            &swap_env.relayer_authority_kp, // authority
            "Coin to PC swap",
            IntegrationStatus::Active,
            1_000_000_000, // rate_limit_slope
            1_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::AtomicSwap(AtomicSwapConfig {
                input_token: swap_env.coin_token_mint,
                output_token: swap_env.pc_token_mint,
                oracle: swap_env.oracle,
                max_slippage_bps: 123,
                max_staleness: 100,
                input_mint_decimals: 6,
                output_mint_decimals: 6,
                expiry_timestamp,
                padding: [0u8; 108],
            }),
            &InitializeArgs::AtomicSwap {
                max_slippage_bps: 123,
                max_staleness: 100,
                expiry_timestamp,
            },
        )?;

        let repay_excess_token_a = false;
        let borrow_amount = 100;
        let repay_amount = 300;

        let integration2_pre = fetch_integration_account(&mut svm, &integration_pk2)?.unwrap();

        atomic_swap_borrow_repay(
            &mut svm,
            &swap_env.relayer_authority_kp,
            swap_env.controller_pk,
            swap_env.permission_pda,
            integration_pk2,
            swap_env.coin_token_mint,
            swap_env.pc_token_mint,
            swap_env.oracle,
            swap_env.price_feed,
            swap_env.relayer_coin, // payer_account_a
            swap_env.relayer_pc,   // payer_account_b
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            borrow_amount,
            repay_amount,
            swap_env.mint_authority.pubkey(),
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

    #[test_log::test]
    fn atomic_swap_rate_limit_violation() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;
        let swap_env = setup_integration_env(&mut svm, expiry_timestamp)?;

        let repay_excess_token_a = false;
        let repay_amount = 30_000_000;

        let integration_pre =
            fetch_integration_account(&mut svm, &swap_env.atomic_swap_integration_pk)?.unwrap();
        let reserve_pc_pre = fetch_reserve_account(&mut svm, &swap_env.pc_reserve_pubkey)?.unwrap();
        let reserve_coin_pre =
            fetch_reserve_account(&mut svm, &swap_env.coin_reserve_pubkey)?.unwrap();

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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            integration_pre.rate_limit_max_outflow + 1,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );
        assert_custom_error(&res, 0, SvmAlmControllerErrors::RateLimited);

        // Initialize a different AtomicSwap integration with higher rate limit than reserve.
        let integration_pk2 = initialize_integration(
            &mut svm,
            &swap_env.controller_pk,
            &swap_env.relayer_authority_kp, // payer
            &swap_env.relayer_authority_kp, // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000_000,                             // rate_limit_slope
            reserve_pc_pre.rate_limit_max_outflow * 2, // rate_limit_max_outflow
            &IntegrationConfig::AtomicSwap(AtomicSwapConfig {
                input_token: swap_env.pc_token_mint,
                output_token: swap_env.coin_token_mint,
                oracle: swap_env.oracle,
                max_slippage_bps: 100,
                max_staleness: 100,
                input_mint_decimals: 6,
                output_mint_decimals: 6,
                expiry_timestamp,
                padding: [0u8; 108],
            }),
            &InitializeArgs::AtomicSwap {
                max_slippage_bps: 100,
                max_staleness: 100,
                expiry_timestamp,
            },
        )?;

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
            pinocchio_token::ID.into(),
            pinocchio_token::ID.into(),
            repay_excess_token_a,
            reserve_pc_pre.rate_limit_max_outflow + 1,
            repay_amount,
            swap_env.mint_authority.pubkey(),
        );
        assert_custom_error(&res, 0, SvmAlmControllerErrors::RateLimited);

        Ok(())
    }
}
