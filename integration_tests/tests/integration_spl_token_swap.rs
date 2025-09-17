mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, initialize_contoller, initialize_integration, initialize_mint,
    initialize_reserve, manage_permission, mint_tokens, push_integration,
};
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus, SplTokenSwapConfig,
};
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {

    use borsh::BorshDeserialize;
    use litesvm::LiteSVM;
    use solana_sdk::{
        instruction::InstructionError,
        program_pack::Pack,
        pubkey::Pubkey,
        transaction::{Transaction, TransactionError},
    };
    use spl_token_2022::{instruction::mint_to, state::Mint};
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::generated::types::{
        AccountingAction, AccountingEvent, IntegrationState, PullArgs, SvmAlmControllerEvent,
    };

    use crate::{
        helpers::{assert::assert_custom_error, constants::TOKEN_SWAP_PROGRAM_ID},
        subs::{
            create_sync_spl_token_swap_ix, derive_controller_authority_pda,
            fetch_integration_account, fetch_spl_token_swap_account, fetch_token_account,
            initialize_swap, pull_integration,
        },
    };
    use test_case::test_case;

    use super::*;

    struct TestContext {
        pub svm: LiteSVM,
        pub authority: Keypair,
        pub usds_mint: Pubkey,
        pub susds_mint: Pubkey,
        pub controller_pk: Pubkey,
        pub usds_reserve_pk: Pubkey,
        pub susds_reserve_pk: Pubkey,
        pub usds_susds_swap_pk: Pubkey,
        pub usds_susds_lp_mint_pk: Pubkey,
        pub usds_susds_lp_vault_pk: Pubkey,
        pub usdc_external_integration_pk: Pubkey,
        pub pool_liquidity_a: u64,
        pub pool_liquidity_b: u64,
    }

    fn setup(permit_liquidation: bool) -> Result<TestContext, Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        // Initialize a mint
        let usds_mint = initialize_mint(
            &mut svm,
            &authority,
            &authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        // Initialize a mint
        let susds_mint = initialize_mint(
            &mut svm,
            &authority,
            &authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Update the authority to have all permissions
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &authority.pubkey(), // subject authority
            PermissionStatus::Active,
            true, // can_execute_swap,
            true, // can_manage_permissions,
            true, // can_invoke_external_transfer,
            true, // can_reallocate,
            true, // can_freeze,
            true, // can_unfreeze,
            true, // can_manage_reserves_and_integrations
            true, // can_suspend_permissions
            true, // can_liquidate
        )?;

        // Initialize a reserve for the USDS token
        let usds_reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &usds_mint, // mint
            &authority, // payer
            &authority, // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;
        let usds_reserve_pk = usds_reserve_keys.pubkey;

        // Initialize a reserve for the sUSDS token
        let susds_reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &susds_mint, // mint
            &authority,  // payer
            &authority,  // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;
        let susds_reserve_pk = susds_reserve_keys.pubkey;

        // Mint a supply of both tokens to the authority -- needed to init the swap
        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &usds_mint,
            &authority.pubkey(),
            1_000_000, // 1
        )?;
        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &susds_mint,
            &authority.pubkey(),
            1_000_000, // 1
        )?;

        // Mint a supply of both tokens into the reserves
        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &usds_mint,
            &controller_authority,
            1_000_000_000, // 1k
        )?;
        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &susds_mint,
            &controller_authority,
            1_000_000_000, // 1k
        )?;

        let pool_liquidity_a = 1_000_000;
        let pool_liquidity_b = 1_000_000;

        // Initialize a token swap for the pair
        let (usds_susds_swap_pk, usds_susds_lp_mint_pk) = initialize_swap(
            &mut svm,
            &authority,
            &authority,
            &usds_mint,
            &susds_mint,
            &TOKEN_SWAP_PROGRAM_ID,
            pool_liquidity_a,
            pool_liquidity_b,
        )?;

        // Initialize an Integration

        let usds_susds_lp_vault_pk = svm_alm_controller_client::derive_spl_token_swap_lp_pda(
            &controller_pk,
            &usds_susds_lp_mint_pk,
        );

        let usdc_external_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // authority
            "USDS/sUSDS Token Swap",
            IntegrationStatus::Active,
            1_000_000_000_000,  // rate_limit_slope
            1_000_000_000_000,  // rate_limit_max_outflow
            permit_liquidation, // permit_liquidation
            &IntegrationConfig::SplTokenSwap(SplTokenSwapConfig {
                program: TOKEN_SWAP_PROGRAM_ID,
                swap: usds_susds_swap_pk,
                mint_a: usds_mint,
                mint_b: susds_mint,
                lp_mint: usds_susds_lp_mint_pk,
                lp_token_account: usds_susds_lp_vault_pk,
                padding: [0; 32],
            }),
            &InitializeArgs::SplTokenSwap,
            false,
        ).map_err(|e| e.err.to_string())?;

        Ok(TestContext {
            svm,
            authority,
            usds_mint,
            susds_mint,
            controller_pk,
            usds_reserve_pk,
            susds_reserve_pk,
            usds_susds_swap_pk,
            usds_susds_lp_mint_pk,
            usds_susds_lp_vault_pk,
            usdc_external_integration_pk,
            pool_liquidity_a,
            pool_liquidity_b,
        })
    }

    #[tokio::test]

    async fn spl_token_swap_push_pull_sync_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            authority,
            usds_mint,
            susds_mint,
            controller_pk,
            usds_reserve_pk,
            susds_reserve_pk,
            usds_susds_swap_pk,
            usds_susds_lp_mint_pk,
            usds_susds_lp_vault_pk,
            usdc_external_integration_pk,
            mut pool_liquidity_a,
            mut pool_liquidity_b,
        } = setup(false).unwrap();

        let (tx_res, _) = push_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            &PushArgs::SplTokenSwap {
                amount_a: 100_000_000,
                amount_b: 120_000_000,
                minimum_pool_token_amount_a: u64::MAX,
                minimum_pool_token_amount_b: u64::MAX,
            },
            true,
        )
        .await?;
        assert!(
            tx_res.is_err(),
            "TX should have errored with too much slippage"
        );

        // Push the integration -- Add Liquidity to the swap pool
        let integration_liquidity_a = 100_000_000;
        pool_liquidity_a += integration_liquidity_a;
        let integration_liquidity_b = 120_000_000;
        pool_liquidity_b += integration_liquidity_b;
        let (tx_result, account_keys) = push_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            &PushArgs::SplTokenSwap {
                amount_a: integration_liquidity_a,
                amount_b: integration_liquidity_b,
                minimum_pool_token_amount_a: 0,
                minimum_pool_token_amount_b: 0,
            },
            false,
        )
        .await?;
        let tx_meta = tx_result.unwrap();

        // Validate that the Sync AccountingEvents were fired during push
        let expected_usds_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            // TODO this may need to change away from Reserve's Pubkey
            integration: usds_reserve_pk,
            mint: usds_mint,
            action: AccountingAction::Sync,
            before: 0,
            after: 1_000_000_000,
        });
        let expected_susds_event = SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: controller_pk,
            // TODO this may need to change away from Reserve's Pubkey
            integration: susds_reserve_pk,
            mint: susds_mint,
            action: AccountingAction::Sync,
            before: 0,
            after: 1_000_000_000,
        });

        assert_contains_controller_cpi_event!(tx_meta, account_keys, expected_usds_event);
        assert_contains_controller_cpi_event!(tx_meta, account_keys, expected_susds_event);

        let tx_res = pull_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            &PullArgs::SplTokenSwap {
                amount_a: 50_000_000,
                amount_b: 60_000_000,
                maximum_pool_token_amount_a: 0,
                maximum_pool_token_amount_b: 0,
            },
            true,
        )?;
        assert!(
            tx_res.is_err(),
            "TX should have errored with too much slippage"
        );

        // Pull the integration -- Withdraw liquidity from the swap pool
        let integration_withdraw_liquidity_a = 50_000_000;
        pool_liquidity_a -= integration_withdraw_liquidity_a;
        let integration_withdraw_liquidity_b = 60_000_000;
        pool_liquidity_b -= integration_withdraw_liquidity_b;
        pull_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            &PullArgs::SplTokenSwap {
                amount_a: integration_withdraw_liquidity_a,
                amount_b: integration_withdraw_liquidity_b,
                maximum_pool_token_amount_a: u64::MAX,
                maximum_pool_token_amount_b: u64::MAX,
            },
            false,
        )?
        .unwrap();

        let pool = fetch_spl_token_swap_account(&svm, &usds_susds_swap_pk)
            .unwrap()
            .unwrap();
        let lp_mint_acct = svm.get_account(&usds_susds_lp_mint_pk).unwrap();
        let lp_mint = Mint::unpack(&lp_mint_acct.data).unwrap();

        let integration_before = fetch_integration_account(&svm, &usdc_external_integration_pk)
            .unwrap()
            .unwrap();
        let integration_state_before = match &integration_before.state {
            IntegrationState::SplTokenSwap(state) => state,
            _ => panic!("Invalid integration state"),
        };

        let sync_ix = create_sync_spl_token_swap_ix(
            &controller_pk,
            &usdc_external_integration_pk,
            &usds_susds_swap_pk,
            &usds_susds_lp_mint_pk,
            &usds_susds_lp_vault_pk,
            &pool.token_a,
            &pool.token_b,
        );

        // Sync TX with unchanged IntegrationState should error to
        // prevent DOS on Integration account.
        let txn = Transaction::new_signed_with_payer(
            &[sync_ix.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(
            &tx_result,
            0,
            SvmAlmControllerErrors::DataNotChangedSinceLastSync,
        );

        // Mint USDS (aka token A to Liquidity prior to sync)
        let pool_token_a_increase = 10_000_000;
        pool_liquidity_a += pool_token_a_increase;
        let mint_a_ix = mint_to(
            &spl_token::ID,
            &usds_mint,
            &pool.token_a,
            &authority.pubkey(),
            &[&authority.pubkey()],
            pool_token_a_increase,
        )?;
        let txn = Transaction::new_signed_with_payer(
            &[mint_a_ix, sync_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        svm.send_transaction(txn).unwrap();

        let lp_vault_account_after = fetch_token_account(&svm, &usds_susds_lp_vault_pk);
        let pool_token_a = fetch_token_account(&svm, &pool.token_a);
        let pool_token_b = fetch_token_account(&svm, &pool.token_b);

        assert_eq!(pool_token_a.amount, pool_liquidity_a);
        assert_eq!(pool_token_b.amount, pool_liquidity_b);

        let expected_token_a_balance =
            pool_liquidity_a * lp_vault_account_after.amount / lp_mint.supply;
        let expected_token_b_balance =
            pool_liquidity_b * lp_vault_account_after.amount / lp_mint.supply;

        let integration_after = fetch_integration_account(&svm, &usdc_external_integration_pk)
            .unwrap()
            .unwrap();
        let integration_state_after = match &integration_after.state {
            IntegrationState::SplTokenSwap(state) => state,
            _ => panic!("Invalid integration state"),
        };

        // Assert Integration state changes
        assert_eq!(
            integration_state_before.last_balance_lp, integration_state_after.last_balance_lp,
            "LP balance should not change"
        );
        assert_eq!(
            expected_token_a_balance,
            integration_state_after.last_balance_a,
        );
        assert_eq!(
            expected_token_b_balance,
            integration_state_after.last_balance_b,
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
    #[test_case(false, false, false, false, false, false, false, false, true, true, false; "can_liquidate w/ permit_liquidation fails")]
    #[tokio::test]
    async fn spl_token_swap_push_permissions(
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
            authority: super_authority,
            usds_mint: _,
            susds_mint: _,
            controller_pk,
            usds_reserve_pk: _,
            susds_reserve_pk: _,
            usds_susds_swap_pk: _,
            usds_susds_lp_mint_pk: _,
            usds_susds_lp_vault_pk: _,
            usdc_external_integration_pk,
            pool_liquidity_a: _,
            pool_liquidity_b: _,
        } = setup(permit_liquidation).unwrap();

        let push_authority = Keypair::new();
        airdrop_lamports(&mut svm, &push_authority.pubkey(), 1_000_000_000)?;
        // Update the authority to have permissions
        let _ = manage_permission(
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

        let (tx_res, _) = push_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &push_authority,
            &PushArgs::SplTokenSwap {
                amount_a: 100_000_000,
                amount_b: 120_000_000,
                minimum_pool_token_amount_a: 0,
                minimum_pool_token_amount_b: 0,
            },
            true,
        )
        .await?;

        // Assert the expected result given the enabled privilege
        match result_ok {
            true => assert!(tx_res.is_ok()),
            false => assert_eq!(
                tx_res.err().unwrap().err,
                TransactionError::InstructionError(2, InstructionError::IncorrectAuthority)
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
    #[tokio::test]
    async fn spl_token_swap_pull_permissions(
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
            authority: super_authority,
            usds_mint: _,
            susds_mint: _,
            controller_pk,
            usds_reserve_pk: _,
            susds_reserve_pk: _,
            usds_susds_swap_pk: _,
            usds_susds_lp_mint_pk: _,
            usds_susds_lp_vault_pk: _,
            usdc_external_integration_pk,
            pool_liquidity_a: _,
            pool_liquidity_b: _,
        } = setup(permit_liquidation).unwrap();

        // Send some tokens into the integration prior to the pull
        push_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &super_authority,
            &PushArgs::SplTokenSwap {
                amount_a: 100_000_000,
                amount_b: 120_000_000,
                minimum_pool_token_amount_a: 0,
                minimum_pool_token_amount_b: 0,
            },
            false,
        )
        .await?
        .0
        .unwrap();

        let pull_authority = Keypair::new();
        airdrop_lamports(&mut svm, &pull_authority.pubkey(), 1_000_000_000)?;
        // Update the authority to have permissions
        let _ = manage_permission(
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

        let tx_res = pull_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &pull_authority,
            &PullArgs::SplTokenSwap {
                amount_a: 50_000_000,
                amount_b: 50_000_000,
                maximum_pool_token_amount_a: u64::MAX,
                maximum_pool_token_amount_b: u64::MAX,
            },
            true,
        )?;

        // Assert the expected result given the enabled privilege
        match result_ok {
            true => assert!(tx_res.is_ok()),
            false => assert_eq!(
                tx_res.err().unwrap().err,
                TransactionError::InstructionError(2, InstructionError::IncorrectAuthority)
            ),
        }

        Ok(())
    }
}
