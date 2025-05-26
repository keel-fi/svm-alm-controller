#[cfg(test)]
mod tests {
    use crate::{
        helpers::spl::setup_token_account,
        subs::{
            atomic_swap_borrow, cancel_atomic_swap, derive_permission_pda,
            fetch_integration_account, fetch_token_account, initialize_reserve, transfer_tokens,
            ReserveKeys,
        },
    };
    use litesvm::LiteSVM;
    use pinocchio_token::state::TokenAccount;
    use solana_sdk::{
        msg, pubkey::Pubkey, signature::Keypair, signer::Signer, system_program,
        transaction::Transaction,
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use spl_token::state::Account;
    use svm_alm_controller_client::generated::{
        accounts::Integration,
        types::{
            AtomicSwapConfig, ControllerStatus, InitializeArgs, IntegrationConfig,
            IntegrationStatus, PermissionStatus, ReserveStatus,
        },
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

    #[test_log::test]
    fn test_happy_path_init_and_cancel_swap() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let relayer_authority_kp = Keypair::new();
        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        // Initialize price feed and oracle.
        let update_slot = 1000_000;
        svm.warp_to_slot(update_slot);
        let nonce = Pubkey::new_unique();
        let price_feed = Pubkey::new_unique();
        set_price_feed(&mut svm, &price_feed, 1_000_000_000)?;
        initalize_oracle(&mut svm, &relayer_authority_kp, &nonce, &price_feed, 0)?;

        let coin_token_mint = Pubkey::new_unique();
        let pc_token_mint = Pubkey::new_unique();
        let mint_authority = Keypair::new();
        setup_token_mint(&mut svm, &coin_token_mint, 6, &mint_authority.pubkey());
        setup_token_mint(&mut svm, &pc_token_mint, 6, &mint_authority.pubkey());

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let _ = manage_permission(
            &mut svm,
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
        let atomic_swap_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::AtomicSwap(AtomicSwapConfig {
                input_token: pc_token_mint,
                output_token: coin_token_mint,
                oracle: derive_oracle_pda(&nonce),
                max_slippage_bps: 123,
                is_input_token_base_asset: true,
                padding: [0u8; 93],
            }),
            &InitializeArgs::AtomicSwap {
                max_slippage_bps: 123,
                is_input_token_base_asset: true,
            },
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
    fn test_happy_path_atomic_swap() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let relayer_authority_kp = Keypair::new();
        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        // Initialize price feed and oracle.
        let update_slot = 1000_000;
        svm.warp_to_slot(update_slot);
        let nonce = Pubkey::new_unique();
        let price_feed = Pubkey::new_unique();
        set_price_feed(&mut svm, &price_feed, 1_000_000_000)?;
        initalize_oracle(&mut svm, &relayer_authority_kp, &nonce, &price_feed, 0)?;

        let coin_token_mint = Pubkey::new_unique();
        let pc_token_mint = Pubkey::new_unique();
        let mint_authority = Keypair::new();
        setup_token_mint(&mut svm, &coin_token_mint, 6, &mint_authority.pubkey());
        setup_token_mint(&mut svm, &pc_token_mint, 6, &mint_authority.pubkey());

        // Setup relayer with funded token accounts.
        let relayer_pc = get_associated_token_address_with_program_id(
            &relayer_authority_kp.pubkey(),
            &pc_token_mint,
            &pinocchio_token::ID.into(),
        );
        setup_token_account(
            &mut svm,
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
            &mut svm,
            &relayer_coin,
            &coin_token_mint,
            &relayer_authority_kp.pubkey(),
            1000_000_000,
            &pinocchio_token::ID.into(),
            None,
        );

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let _ = manage_permission(
            &mut svm,
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
        let atomic_swap_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp, // payer
            &relayer_authority_kp, // authority
            "Pc to Coin swap",
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::AtomicSwap(AtomicSwapConfig {
                input_token: pc_token_mint,
                output_token: coin_token_mint,
                oracle: derive_oracle_pda(&nonce),
                max_slippage_bps: 123,
                is_input_token_base_asset: true,
                padding: [0u8; 93],
            }),
            &InitializeArgs::AtomicSwap {
                max_slippage_bps: 123,
                is_input_token_base_asset: true,
            },
        )?;
        let integration = fetch_integration_account(&mut svm, &atomic_swap_integration_pk)?;
        assert!(integration.is_some(), "Integration account is not found");

        let ReserveKeys {
            pubkey: pc_reserve_pubkey,
            vault: pc_reserve_vault,
        } = initialize_reserve(
            &mut svm,
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
            &mut svm,
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
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            &pc_token_mint,
            &controller_pk,
            300_000_000,
        )?;
        transfer_tokens(
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            &coin_token_mint,
            &controller_pk,
            600_000_000,
        )?;

        let calling_permission_pda =
            derive_permission_pda(&controller_pk, &relayer_authority_kp.pubkey());

        // atomic_swap_borrow(
        //     &mut svm,
        //     &relayer_authority_kp,
        //     controller_pk,
        //     calling_permission_pda,
        //     atomic_swap_integration_pk,
        //     pc_token_mint,
        //     coin_token_mint,
        //     relayer_pc,
        //     100,
        // )?;
        // let ta = fetch_token_account(&mut svm, &relayer_pc);
        // println!("amount: {:?}", ta.amount);
        // assert!(false);

        Ok(())
    }
}
