#[cfg(test)]
mod tests {
    use crate::{
        helpers::spl::setup_token_account,
        subs::{
            atomic_swap_borrow_repay, cancel_atomic_swap, derive_permission_pda,
            fetch_integration_account, fetch_token_account, initialize_reserve, transfer_tokens,
            ReserveKeys,
        },
    };
    use litesvm::LiteSVM;
    use solana_sdk::{clock::Clock, pubkey::Pubkey, signature::Keypair, signer::Signer};
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use svm_alm_controller_client::generated::types::{
        AtomicSwapConfig, ControllerStatus, InitializeArgs, IntegrationConfig, IntegrationStatus,
        PermissionStatus, ReserveStatus,
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

    fn setup_integration_env(
        svm: &mut LiteSVM,
        relayer_authority_kp: &Keypair,
        price_feed: &Pubkey,
        nonce: &Pubkey,
        coin_token_mint: &Pubkey,
        pc_token_mint: &Pubkey,
        mint_authority: &Keypair,
        expiry_timestamp: i64,
    ) -> Result<
        (
            Pubkey,
            Pubkey,
            Pubkey,
            Pubkey,
            Pubkey,
            Pubkey,
            Pubkey,
            Pubkey,
        ),
        Box<dyn std::error::Error>,
    > {
        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        // Initialize price feed and oracle.
        let update_slot = 1000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(svm, price_feed, 1_000_000_000)?;
        initalize_oracle(svm, &relayer_authority_kp, nonce, price_feed, 0)?;

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
        let _ = manage_permission(
            svm,
            &controller_pk,
            &relayer_authority_kp,          // payer
            &relayer_authority_kp,          // calling authority
            &relayer_authority_kp.pubkey(), // subject authority
            PermissionStatus::Active,
            true,  // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            true,  // can_manage_integrations
        )?;

        // Initialize an AtomicSwap integration
        let oracle = derive_oracle_pda(&nonce);
        let atomic_swap_integration_pk = initialize_integration(
            svm,
            &controller_pk,
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::AtomicSwap(AtomicSwapConfig {
                input_token: *pc_token_mint,
                output_token: *coin_token_mint,
                oracle,
                max_slippage_bps: 123,
                is_input_token_base_asset: true,
                max_staleness: 100,
                input_mint_decimals: 6,
                output_mint_decimals: 6,
                expiry_timestamp,
                padding: [0u8; 75],
            }),
            &InitializeArgs::AtomicSwap {
                max_slippage_bps: 123,
                is_input_token_base_asset: true,
                max_staleness: 100,
                expiry_timestamp,
            },
        )?;

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
            1000_000_000,
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
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
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
            &controller_pk,
            300_000_000,
        )?;
        transfer_tokens(
            svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            &coin_token_mint,
            &controller_pk,
            600_000_000,
        )?;

        Ok((
            controller_pk,
            atomic_swap_integration_pk,
            relayer_pc,
            relayer_coin,
            pc_reserve_pubkey,
            pc_reserve_vault,
            coin_reserve_pubkey,
            coin_reserve_vault,
        ))
    }

    #[test_log::test]
    fn init_and_cancel_atomic_swap() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let relayer_authority_kp = Keypair::new();
        let price_feed = Pubkey::new_unique();
        let nonce = Pubkey::new_unique();
        let coin_token_mint = Pubkey::new_unique();
        let pc_token_mint = Pubkey::new_unique();
        let mint_authority = Keypair::new();
        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;

        let (
            controller_pk,
            atomic_swap_integration_pk,
            _relayer_pc,
            _relayer_coin,
            _pc_reserve_pubkey,
            _pc_reserve_vault,
            _coin_reserve_pubkey,
            _coin_reserve_vault,
        ) = setup_integration_env(
            &mut svm,
            &relayer_authority_kp,
            &price_feed,
            &nonce,
            &coin_token_mint,
            &pc_token_mint,
            &mint_authority,
            expiry_timestamp,
        )?;
        let integration = fetch_integration_account(&mut svm, &atomic_swap_integration_pk)?;
        assert!(integration.is_some(), "Integration account is not found");

        let calling_permission_pda =
            derive_permission_pda(&controller_pk, &relayer_authority_kp.pubkey());

        cancel_atomic_swap(
            &mut svm,
            &relayer_authority_kp,
            controller_pk,
            calling_permission_pda,
            atomic_swap_integration_pk,
        )?;
        let integration = fetch_integration_account(&mut svm, &atomic_swap_integration_pk)?;
        assert!(integration.is_none(), "Integration account is found");

        Ok(())
    }

    #[test_log::test]
    fn atomic_swap_success() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let relayer_authority_kp = Keypair::new();
        let price_feed = Pubkey::new_unique();
        let nonce = Pubkey::new_unique();
        let coin_token_mint = Pubkey::new_unique();
        let pc_token_mint = Pubkey::new_unique();
        let mint_authority = Keypair::new();
        let oracle = derive_oracle_pda(&nonce);
        let expiry_timestamp = svm.get_sysvar::<Clock>().unix_timestamp + 1000;

        let (
            controller_pk,
            atomic_swap_integration_pk,
            relayer_pc,
            relayer_coin,
            _pc_reserve_pubkey,
            pc_reserve_vault,
            _coin_reserve_pubkey,
            coin_reserve_vault,
        ) = setup_integration_env(
            &mut svm,
            &relayer_authority_kp,
            &price_feed,
            &nonce,
            &coin_token_mint,
            &pc_token_mint,
            &mint_authority,
            expiry_timestamp,
        )?;
        let integration = fetch_integration_account(&mut svm, &atomic_swap_integration_pk)?;
        assert!(integration.is_some(), "Integration account is not found");

        let vault_a_before = fetch_token_account(&mut svm, &pc_reserve_vault);
        let vault_b_before = fetch_token_account(&mut svm, &coin_reserve_vault);
        let relayer_a_before = fetch_token_account(&mut svm, &relayer_pc);
        let relayer_b_before = fetch_token_account(&mut svm, &relayer_coin);

        let calling_permission_pda =
            derive_permission_pda(&controller_pk, &relayer_authority_kp.pubkey());

        let borrow_amount = 100;
        let repay_amount = 300;

        atomic_swap_borrow_repay(
            &mut svm,
            &relayer_authority_kp,
            controller_pk,
            calling_permission_pda,
            atomic_swap_integration_pk,
            pc_token_mint,
            coin_token_mint,
            oracle,
            price_feed,
            relayer_pc,   // recipient
            relayer_coin, // payer
            borrow_amount,
            repay_amount,
        )?;

        let vault_a_after = fetch_token_account(&mut svm, &pc_reserve_vault);
        let vault_b_after = fetch_token_account(&mut svm, &coin_reserve_vault);
        let relayer_a_after = fetch_token_account(&mut svm, &relayer_pc);
        let relayer_b_after = fetch_token_account(&mut svm, &relayer_coin);

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

        // Check that integration is closed
        let integration = fetch_integration_account(&mut svm, &atomic_swap_integration_pk)?;
        assert!(integration.is_none(), "Integration account is found");

        Ok(())
    }
}
