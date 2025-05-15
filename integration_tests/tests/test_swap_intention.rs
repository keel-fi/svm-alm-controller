mod helpers;
mod subs;

use helpers::raydium::RAYDIUM_LEGACY_AMM_V4;
use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;

fn lite_svm_with_programs() -> LiteSVM {
    let mut svm = LiteSVM::new();

    // Add the CONTROLLER program
    let controller_program_bytes = include_bytes!("../../target/deploy/svm_alm_controller.so");
    svm.add_program(
        svm_alm_controller_client::programs::SVM_ALM_CONTROLLER_ID,
        controller_program_bytes,
    );

    // Add the Orca SWAP program
    let raydium_swap_v4 = include_bytes!("../fixtures/raydium_amm_legacy_v4.so");
    svm.add_program(RAYDIUM_LEGACY_AMM_V4, raydium_swap_v4);

    svm
}

#[cfg(test)]
mod tests {
    use solana_sdk::{signature::Keypair, signer::Signer};
    use svm_alm_controller_client::types::{ControllerStatus, PermissionStatus, ReserveStatus};

    use crate::{
        helpers::{
            raydium::{setup_amm, setup_amm_config, swap_base_in},
            spl::{setup_token_account, setup_token_mint, SPL_TOKEN_PROGRAM_ID},
        },
        subs::{
            edit_token_amount, initialize_contoller, initialize_reserve, manage_permission,
            ReserveKeys,
        },
    };

    use super::*;

    #[test]
    fn test_basic_swap_through_raydium_v4() -> Result<(), Box<dyn std::error::Error>> {
        let coin_liquidity: u64 = 10_000_000;
        let pc_liquidity: u64 = 10_000_000;
        let amount_in: u64 = 500;
        let _expected_amount_out: u64 = 499;

        let mut svm = lite_svm_with_programs();

        let relayer_authority_kp = Keypair::new();
        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();
        setup_amm_config(&mut svm);
        let coin_token_mint = Pubkey::new_unique();
        let pc_token_mint = Pubkey::new_unique();
        let mint_authority = Keypair::new();
        setup_token_mint(&mut svm, &coin_token_mint, 6, &mint_authority.pubkey());
        setup_token_mint(&mut svm, &pc_token_mint, 6, &mint_authority.pubkey());

        let amm_accounts = setup_amm(
            &mut svm,
            coin_token_mint,
            pc_token_mint,
            coin_liquidity,
            pc_liquidity,
        );
        let initial_source_amount = 1_000;

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;

        // REVIEW: What is calling_authority vs subject_authority?
        // Update the authority to have all permissions
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp,          // payer
            &relayer_authority_kp,          // calling authority
            &relayer_authority_kp.pubkey(), // subject authority
            PermissionStatus::Active,
            true, // can_execute_swap,
            true, // can_manage_permissions,
            true, // can_invoke_external_transfer,
            true, // can_reallocate,
            true, // can_freeze,
            true, // can_unfreeze,
            true, // can_manage_integrations
        )?;

        // Initialize a reserve for the token
        let ReserveKeys {
            pubkey: _pc_reserve_pubkey,
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

        // Put some funds in the reserve's vault
        edit_token_amount(&mut svm, &pc_reserve_vault, initial_source_amount)?;
        // Initialize the coin vault, for receiving balance
        let ReserveKeys {
            pubkey: _coin_reserve_pubkey,
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
        // TODO: stub the OracleAccount data

        let _swap_base_in_ix = swap_base_in(
            &RAYDIUM_LEGACY_AMM_V4,
            &amm_accounts.amm,
            &amm_accounts.amm_authority,
            &amm_accounts.open_orders,
            &amm_accounts.coin_vault,
            &amm_accounts.pc_vault,
            &amm_accounts.market_program,
            &amm_accounts.market,
            &amm_accounts.market_bids,
            &amm_accounts.market_asks,
            &amm_accounts.market_event_queue,
            &amm_accounts.market_coin_vault,
            &amm_accounts.market_pc_vault,
            &amm_accounts.market_vault_signer,
            &pc_reserve_vault,
            &coin_reserve_vault,
            &controller_pk,
            amount_in,
            // allow for any slippage
            0,
        )
        .unwrap();
        // TODO: sandwhich the swap instruction with the setup and clean up

        // TODO: Write assertions for ALM controller balance changes
        Ok(())
    }
}
