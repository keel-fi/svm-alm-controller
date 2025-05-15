mod helpers;

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

    use crate::helpers::{
        raydium::{setup_amm, setup_amm_config, swap_base_in},
        spl::{setup_token_account, setup_token_mint, SPL_TOKEN_PROGRAM_ID},
    };

    use super::*;

    #[test]
    fn test_basic_swap_through_raydium_v4() {
        let coin_liquidity: u64 = 10_000_000;
        let pc_liquidity: u64 = 10_000_000;
        let amount_in: u64 = 500;
        let _expected_amount_out: u64 = 499;

        let user_kp = Keypair::new();
        let mut svm = lite_svm_with_programs();
        svm.airdrop(&user_kp.pubkey(), 100_000_000).unwrap();
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
        // Mock user account for tokens in
        let user_token_source = Pubkey::new_unique();
        let initial_source_amount = 1_000;
        setup_token_account(
            &mut svm,
            &user_token_source,
            &pc_token_mint,
            &user_kp.pubkey(),
            initial_source_amount,
            &SPL_TOKEN_PROGRAM_ID,
            None,
        );
        let user_token_destination = Pubkey::new_unique();
        setup_token_account(
            &mut svm,
            &user_token_destination,
            &coin_token_mint,
            &user_kp.pubkey(),
            0,
            &SPL_TOKEN_PROGRAM_ID,
            None,
        );

        // TODO: Set up a controller and relayer with swap capabilities.
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
            &user_token_source,
            &user_token_destination,
            &user_kp.pubkey(),
            amount_in,
            // allow for any slippage
            0,
        )
        .unwrap();
        // TODO: sandwhich the swap instruction with the setup and clean up

        // TODO: Write assertions for ALM controller balance changes


    }
}
