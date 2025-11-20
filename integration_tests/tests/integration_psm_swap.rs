mod helpers;
mod subs;

#[cfg(test)]
mod tests {
    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{setup_test_controller, spl::SPL_TOKEN_PROGRAM_ID, TestContext},
        subs::{
            derive_controller_authority_pda, derive_reserve_pda, edit_token_amount,
            fetch_integration_account, fetch_reserve_account, get_token_balance_or_zero,
            initialize_mint, initialize_reserve, mint_tokens, setup_pool_with_token,
            transfer_tokens,
        },
        test_invalid_accounts,
    };
    use borsh::BorshDeserialize;
    use borsh::BorshSerialize;
    use litesvm::LiteSVM;
    use psm_client::accounts::{PsmPool, Token};
    use solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use spl_token::ID as TOKEN_PROGRAM_ID;
    use svm_alm_controller_client::{
        generated::types::{
            AccountingAction, AccountingDirection, AccountingEvent, IntegrationConfig,
            IntegrationState, IntegrationStatus, IntegrationUpdateEvent, PsmSwapConfig,
            ReserveStatus, SvmAlmControllerEvent,
        },
        initialize_integration::create_psm_swap_initialize_integration_instruction,
        pull::psm_swap::create_psm_swap_pull_instruction,
        push::create_psm_swap_push_instruction,
        sync_integration::create_psm_swap_sync_integration_instruction,
    };
    use test_case::test_case;

    fn serialize_to_vec<T: BorshSerialize>(data: &T) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = Vec::new();
        data.serialize(&mut buf)?;
        Ok(buf)
    }

    fn setup_sync_test() -> Result<
        (
            LiteSVM,
            Pubkey,  // controller_pk
            Keypair, // super_authority
            Pubkey,  // liquidity_mint
            Pubkey,  // pool_pda
            Pubkey,  // token_pda
            Pubkey,  // token_vault
            Pubkey,  // psm_token_vault
            Pubkey,  // integration_pda
            PsmSwapConfig,
        ),
        Box<dyn std::error::Error>,
    > {
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
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        // Initialize reserve first (with its own vault)
        let _reserve_keys = crate::subs::reserve::initialize_reserve(
            &mut svm,
            &controller_pk,
            &liquidity_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            10_000_000_000,
            10_000_000_000,
            &TOKEN_PROGRAM_ID,
        )?;

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );

        // Read the PSM token account to get the vault
        let psm_token_acc = svm.get_account(&token_pda).unwrap();
        let psm_token = Token::from_bytes(&psm_token_acc.data[1..])?;
        let psm_token_vault = psm_token.vault;

        let (init_psm_integration_ix, integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        svm.send_transaction(transaction).unwrap();

        // Fetch integration to get config
        let integration = fetch_integration_account(&svm, &integration_pda)
            .expect("integration should exist")
            .unwrap();

        let psm_config = match integration.config {
            IntegrationConfig::PsmSwap(config) => config,
            _ => panic!("invalid config"),
        };

        Ok((
            svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            pool_pda,
            token_pda,
            token_vault,
            psm_token_vault,
            integration_pda,
            psm_config,
        ))
    }

    fn initialize_psm_swap_integration(
        token_program: &Pubkey,
    ) -> Result<
        (
            LiteSVM,
            Pubkey,  // controller pda
            Keypair, // super_authority
            Pubkey,  // mint
            Pubkey,  // pool_pda
            Pubkey,  // token_pda
            Pubkey,  // token_vault
            Pubkey,  // integration pda
        ),
        Box<dyn std::error::Error>,
    > {
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
            token_program,
            None,
            None,
        )?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &liquidity_mint,
            &super_authority.pubkey(),
            1_000_000_000_000,
        )?;

        // initialize a reserve for the token
        initialize_reserve(
            &mut svm,
            &controller_pk,
            &liquidity_mint,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            token_program,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &liquidity_mint,
            &controller_authority,
            1_000_000_000_000,
        )?;

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            if token_program == &TOKEN_PROGRAM_ID {
                false
            } else {
                true
            },
            false,
            &controller_authority,
        );

        let (init_psm_integration_ix, integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        svm.send_transaction(transaction.clone()).unwrap();

        Ok((
            svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            pool_pda,
            token_pda,
            token_vault,
            integration_pda,
        ))
    }

    #[test]
    fn test_psm_swap_init_success() -> Result<(), Box<dyn std::error::Error>> {
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
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );
        let rate_limit_slope = 10_000_000_000;
        let rate_limit_max_outflow = 10_000_000_000;
        let integration_status = IntegrationStatus::Active;
        let permit_liquidation = true;
        let description = "psm swap";

        let (init_psm_integration_ix, integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                description,
                integration_status,
                rate_limit_slope,
                rate_limit_max_outflow,
                permit_liquidation,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let integration = fetch_integration_account(&svm, &integration_pda)
            .expect("integration should exist")
            .unwrap();

        let clock = svm.get_sysvar::<Clock>();

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
            IntegrationConfig::PsmSwap(config) => {
                assert_eq!(
                    config,
                    PsmSwapConfig {
                        psm_token: token_pda,
                        psm_pool: pool_pda,
                        mint: liquidity_mint,
                        padding: [0; 128]
                    }
                );
            }
            _ => panic!("invalid config"),
        }

        let psm_state = match integration.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        assert_eq!(psm_state.liquidity_supplied, 0);

        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: integration_pda,
            authority: super_authority.pubkey(),
            old_state: None,
            new_state: Some(integration),
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            expected_event
        );

        Ok(())
    }

    enum InvalidPsmStates {
        LiquidityOwnerMismatch,
        PoolMismatch,
        MintMismatch,
        VaultMismatch,
    }

    #[test_case(InvalidPsmStates::LiquidityOwnerMismatch; "liquidity owner mismatch")]
    #[test_case(InvalidPsmStates::PoolMismatch; "pool mismatch")]
    #[test_case(InvalidPsmStates::MintMismatch; "mint mismatch")]
    #[test_case(InvalidPsmStates::VaultMismatch; "vault mismatch")]
    fn test_psm_swap_init_invalid_psm_state_fails(
        invalid_state: InvalidPsmStates,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );

        let mut psm_pool_acc = svm.get_account(&pool_pda).unwrap();
        let mut psm_pool = PsmPool::from_bytes(&psm_pool_acc.data[1..])?;
        let mut psm_token_pda_acc = svm.get_account(&token_pda).unwrap();
        let mut psm_token = Token::from_bytes(&psm_token_pda_acc.data[1..])?;
        match invalid_state {
            InvalidPsmStates::LiquidityOwnerMismatch => {
                psm_pool.liquidity_owner = Pubkey::default()
            }
            InvalidPsmStates::MintMismatch => psm_token.mint = Pubkey::default(),
            InvalidPsmStates::PoolMismatch => psm_token.pool = Pubkey::default(),
            InvalidPsmStates::VaultMismatch => psm_token.pool = Pubkey::default(),
        }
        psm_pool_acc.data[1..].copy_from_slice(&serialize_to_vec(&psm_pool)?);
        svm.set_account(pool_pda, psm_pool_acc).unwrap();
        psm_token_pda_acc.data[1..].copy_from_slice(&serialize_to_vec(&psm_token)?);
        svm.set_account(token_pda, psm_token_pda_acc).unwrap();

        let (init_psm_integration_ix, _integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone());

        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );

        Ok(())
    }

    #[test]
    fn test_psm_swap_init_invalid_account_owners_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );

        let (init_psm_integration_ix, _integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            init_psm_integration_ix.clone(),
            {
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "PsmPool invalid owner"),
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "PsmToken invalid owner"),
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "PsmTokenVault invalid owner"),
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint invalid owner"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_psm_swap_init_psm_vault_with_balance() -> Result<(), Box<dyn std::error::Error>> {
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
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );

        let vault_balance = 1_000_000;
        edit_token_amount(&mut svm, &token_vault, vault_balance)?;

        let (init_psm_integration_ix, integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        svm.send_transaction(transaction).unwrap();

        let integration = fetch_integration_account(&svm, &integration_pda)
            .expect("integration should exist")
            .unwrap();

        let psm_state = match integration.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        assert_eq!(psm_state.liquidity_supplied, vault_balance);

        Ok(())
    }

    #[test]
    fn test_psm_swap_sync_success() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            _pool_pda,
            _token_pda,
            token_vault,
            psm_token_vault,
            integration_pda,
            psm_config,
        ) = setup_sync_test()?;

        // Fetch integration before sync
        let integration_before = fetch_integration_account(&svm, &integration_pda)
            .expect("integration should exist")
            .unwrap();

        let psm_state_before = match integration_before.state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };

        // Transfer tokens to the vault (simulating a non-zero balance)
        let vault_balance = 5_000_000;
        edit_token_amount(&mut svm, &token_vault, vault_balance)?;

        // Create sync instruction with the PSM token vault
        let sync_ix = create_psm_swap_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pda,
            &psm_config,
            &liquidity_mint,
            &psm_token_vault,
        )?;

        // Execute sync
        let transaction = Transaction::new_signed_with_payer(
            &[sync_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        // Fetch integration after sync
        let integration_after = fetch_integration_account(&svm, &integration_pda)
            .expect("integration should exist")
            .unwrap();

        // Verify liquidity_supplied was updated
        let psm_state_after = match integration_after.state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };

        // Verify state was synced (should match vault balance)
        assert_eq!(psm_state_after.liquidity_supplied, vault_balance);

        // Calculate delta - final should be greater than initial (0), so delta should be non-zero
        let liquidity_delta = psm_state_after
            .liquidity_supplied
            .abs_diff(psm_state_before.liquidity_supplied);

        // Construct expected accounting event directly from known values
        // Initial state = 0, final state = vault_balance, so delta = vault_balance
        // Direction: Credit since final > initial
        let expected_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pda),
            reserve: None,
            mint: liquidity_mint,
            action: AccountingAction::Sync,
            delta: liquidity_delta,
            direction: AccountingDirection::Credit, // Credit since final > initial
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            expected_event
        );
        // Verify delta is non-zero (success case with actual balance change)
        assert_eq!(liquidity_delta, vault_balance);

        Ok(())
    }

    #[test_case(SPL_TOKEN_PROGRAM_ID; "SPL Token program")]
    #[test_case(spl_token_2022::ID; " Token2022 program")]
    fn test_psm_swap_push_success(token_program: Pubkey) -> Result<(), Box<dyn std::error::Error>> {
        // initialize environment
        let (
            mut svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            pool_pda,
            token_pda,
            token_vault,
            integration_pda,
        ) = initialize_psm_swap_integration(&token_program)?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let reserve_pda = derive_reserve_pda(&controller_pk, &liquidity_mint);
        let reserve_vault = get_associated_token_address_with_program_id(
            &controller_authority,
            &liquidity_mint,
            &token_program,
        );
        // add some extra balance to the token_vault so we trigger all events in push
        let vault_balance_before_push = 1_000_000;
        edit_token_amount(&mut svm, &token_vault, vault_balance_before_push)?;

        let integration_before = fetch_integration_account(&svm, &integration_pda)
            .unwrap()
            .unwrap();

        let reserve_before = fetch_reserve_account(&svm, &reserve_pda).unwrap().unwrap();
        let reserve_balance_before = get_token_balance_or_zero(&svm, &reserve_vault);
        let psm_token_vault_balance_before = get_token_balance_or_zero(&svm, &token_vault);

        let push_amount = 1_000_000;
        let push_ix = create_psm_swap_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &liquidity_mint,
            &integration_pda,
            &token_program,
            &pool_pda,
            &token_pda,
            &token_vault,
            push_amount,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let reserve_after = fetch_reserve_account(&svm, &reserve_pda).unwrap().unwrap();

        let integration_after = fetch_integration_account(&svm, &integration_pda)
            .unwrap()
            .unwrap();

        let reserve_balance_after = get_token_balance_or_zero(&svm, &reserve_vault);
        let psm_token_vault_balance_after = get_token_balance_or_zero(&svm, &token_vault);

        // Assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        assert_eq!(
            state_after.liquidity_supplied,
            psm_token_vault_balance_after
        );
        assert_eq!(
            state_after.liquidity_supplied,
            state_before.liquidity_supplied + push_amount + vault_balance_before_push
        );

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
        assert_eq!(reserve_balance_after, reserve_balance_before - push_amount);

        // Assert PSM Token's token account received the tokens
        assert_eq!(
            psm_token_vault_balance_after,
            psm_token_vault_balance_before + push_amount
        );

        // assert sync event before CPI
        let event_before_cpi = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pda),
            reserve: None,
            mint: liquidity_mint,
            action: AccountingAction::Sync,
            delta: vault_balance_before_push,
            direction: AccountingDirection::Credit,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            event_before_cpi
        );

        // assert credit event after CPI
        let integration_event_after_cpi = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pda),
            mint: liquidity_mint,
            reserve: None,
            direction: AccountingDirection::Credit,
            action: AccountingAction::Deposit,
            delta: psm_token_vault_balance_after - psm_token_vault_balance_before,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            integration_event_after_cpi
        );

        // assert debit event after CPI
        let reserve_event_after_cpi = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            mint: liquidity_mint,
            reserve: Some(reserve_pda),
            direction: AccountingDirection::Debit,
            action: AccountingAction::Deposit,
            delta: reserve_balance_before - reserve_balance_after,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            reserve_event_after_cpi
        );

        Ok(())
    }

    #[test]
    fn test_psm_swap_sync_invalid_accounts_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            _pool_pda,
            _token_pda,
            _token_vault,
            psm_token_vault,
            integration_pda,
            psm_config,
        ) = setup_sync_test()?;

        // Create valid sync instruction
        let sync_ix = create_psm_swap_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pda,
            &psm_config,
            &liquidity_mint,
            &psm_token_vault,
        )?;

        // Test invalid accounts for the inner context accounts (remaining accounts)
        // Account indices: 5=vault, 6=psm_token, 7=psm_pool, 8=mint
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            sync_ix,
            {
                5 => invalid_owner(InstructionError::InvalidAccountOwner, "Vault: invalid owner"),
                5 => invalid_pubkey(InstructionError::InvalidAccountData, "Vault: does not match config"),
                6 => invalid_owner(InstructionError::InvalidAccountOwner, "PSM Token: invalid owner"),
                6 => invalid_pubkey(InstructionError::InvalidAccountData, "PSM Token: does not match config"),
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "PSM Pool: invalid owner"),
                7 => invalid_pubkey(InstructionError::InvalidAccountData, "PSM Pool: does not match config"),
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: invalid owner"),
                8 => invalid_pubkey(InstructionError::InvalidAccountData, "Mint: does not match config"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_psm_swap_sync_invalid_config_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            _pool_pda,
            _token_pda,
            _token_vault,
            psm_token_vault,
            integration_pda,
            psm_config,
        ) = setup_sync_test()?;

        // Create a config with wrong psm_token
        // Use the mint as the wrong psm_token - it exists but has wrong data
        let mut wrong_config = psm_config;
        wrong_config.psm_token = liquidity_mint; // Wrong psm_token (using mint, which exists but has wrong data)

        // Create sync instruction with wrong config
        let sync_ix = create_psm_swap_sync_integration_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &integration_pda,
            &wrong_config,
            &liquidity_mint,
            &psm_token_vault,
        )?;

        // Execute sync - should fail
        let transaction = Transaction::new_signed_with_payer(
            &[sync_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction);

        // When passing wrong mint, the account validation fails with InvalidAccountOwner
        // because the reserve vault derivation doesn't match, or with InvalidAccountData
        // if the account owner check passes but data validation fails
        let error = tx_result.err().unwrap().err;
        assert!(
            matches!(
                error,
                TransactionError::InstructionError(0, InstructionError::InvalidAccountOwner)
                    | TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
            ),
            "Expected InvalidAccountOwner or InvalidAccountData, got: {:?}",
            error
        );

        Ok(())
    }

    enum PsmIx {
        Push,
        Pull,
    }

    #[test_case(PsmIx::Push; "Push Instruction")]
    #[test_case(PsmIx::Pull; "Pull Instruction")]
    fn test_psm_swap_push_pull_invalid_accounts_fails(
        ix: PsmIx,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let token_program = TOKEN_PROGRAM_ID;
        // initialize environment
        let (
            mut svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            pool_pda,
            token_pda,
            token_vault,
            integration_pda,
        ) = initialize_psm_swap_integration(&token_program)?;

        let amount = 1_000_000;
        let push_ix = create_psm_swap_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &liquidity_mint,
            &integration_pda,
            &token_program,
            &pool_pda,
            &token_pda,
            &token_vault,
            amount,
        );

        let ix = match ix {
            PsmIx::Push => push_ix,
            PsmIx::Pull => {
                // first process push
                let transaction = Transaction::new_signed_with_payer(
                    &[push_ix],
                    Some(&super_authority.pubkey()),
                    &[super_authority.insecure_clone()],
                    svm.latest_blockhash(),
                );
                svm.send_transaction(transaction).unwrap();

                create_psm_swap_pull_instruction(
                    &controller_pk,
                    &super_authority.pubkey(),
                    &liquidity_mint,
                    &integration_pda,
                    &token_program,
                    &pool_pda,
                    &token_pda,
                    &token_vault,
                    amount,
                )
            }
        };

        // Test invalid accounts for the inner context accounts (remaining accounts)
        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            ix,
            {
                7 => invalid_owner(InstructionError::InvalidAccountOwner, "PSM Pool: invalid owner"),
                7 => invalid_pubkey(InstructionError::InvalidAccountData, "PSM Pool: does not match config"),
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "PSM Token: invalid owner"),
                8 => invalid_pubkey(InstructionError::InvalidAccountData, "PSM Token: does not match config"),
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "PSM Token Vault: invalid owner"),
                9 => invalid_pubkey(InstructionError::InvalidAccountData, "PSM Token Vault: does not match config"),
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint: invalid owner"),
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Reserve vault: invalid owner"),
                12 => invalid_program_id(InstructionError::IncorrectProgramId, "Token program: invalid pubkey"),
                13 => invalid_program_id(InstructionError::IncorrectProgramId, "Associated token program: invalid pubkey"),
                14 => invalid_program_id(InstructionError::IncorrectProgramId, "PSM program: invalid pubkey"),
            }
        );

        Ok(())
    }

    #[test_case(SPL_TOKEN_PROGRAM_ID; "SPL Token program")]
    #[test_case(spl_token_2022::ID; " Token2022 program")]
    fn test_psm_swap_pull_success(token_program: Pubkey) -> Result<(), Box<dyn std::error::Error>> {
        // initialize environment
        let (
            mut svm,
            controller_pk,
            super_authority,
            liquidity_mint,
            pool_pda,
            token_pda,
            token_vault,
            integration_pda,
        ) = initialize_psm_swap_integration(&token_program)?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let reserve_pda = derive_reserve_pda(&controller_pk, &liquidity_mint);
        let reserve_vault = get_associated_token_address_with_program_id(
            &controller_authority,
            &liquidity_mint,
            &token_program,
        );

        let push_ix = create_psm_swap_push_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &liquidity_mint,
            &integration_pda,
            &token_program,
            &pool_pda,
            &token_pda,
            &token_vault,
            1_000_000,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[push_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        svm.send_transaction(transaction).unwrap();

        let integration_before = fetch_integration_account(&svm, &integration_pda)
            .unwrap()
            .unwrap();

        let reserve_before = fetch_reserve_account(&svm, &reserve_pda).unwrap().unwrap();
        let reserve_balance_before = get_token_balance_or_zero(&svm, &reserve_vault);
        let psm_token_vault_balance_before = get_token_balance_or_zero(&svm, &token_vault);

        let pull_amount = 1_000_000;
        let pull_ix = create_psm_swap_pull_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &liquidity_mint,
            &integration_pda,
            &token_program,
            &pool_pda,
            &token_pda,
            &token_vault,
            pull_amount,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[pull_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let reserve_after = fetch_reserve_account(&svm, &reserve_pda).unwrap().unwrap();

        let integration_after = fetch_integration_account(&svm, &integration_pda)
            .unwrap()
            .unwrap();

        let reserve_balance_after = get_token_balance_or_zero(&svm, &reserve_vault);
        let psm_token_vault_balance_after = get_token_balance_or_zero(&svm, &token_vault);

        // Assert integration state changed
        let state_before = match integration_before.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        let state_after = match integration_after.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        assert_eq!(
            state_after.liquidity_supplied,
            psm_token_vault_balance_after
        );
        assert_eq!(
            state_after.liquidity_supplied,
            state_before.liquidity_supplied - pull_amount
        );

        // Assert Integration rate limits adjusted
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
        assert_eq!(reserve_balance_after, reserve_balance_before + pull_amount);

        // Assert PSM Token's token account transferred the tokens
        assert_eq!(
            psm_token_vault_balance_after,
            psm_token_vault_balance_before - pull_amount
        );

        // assert debit event after CPI
        let integration_event_after_cpi = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: Some(integration_pda),
            mint: liquidity_mint,
            reserve: None,
            direction: AccountingDirection::Debit,
            action: AccountingAction::Withdrawal,
            delta: pull_amount,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            integration_event_after_cpi
        );

        // assert credit event after CPI
        let reserve_event_after_cpi = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            integration: None,
            mint: liquidity_mint,
            reserve: Some(reserve_pda),
            direction: AccountingDirection::Credit,
            action: AccountingAction::Withdrawal,
            delta: pull_amount,
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            reserve_event_after_cpi
        );

        Ok(())
    }
}
