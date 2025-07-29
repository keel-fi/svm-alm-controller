mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, initialize_contoller, initialize_integration, initialize_mint,
    initialize_reserve, manage_permission, mint_tokens, push_integration,
};
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus, SplTokenSwapConfig,
};
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {

    use svm_alm_controller_client::generated::types::PullArgs;

    use crate::{
        helpers::constants::NOVA_TOKEN_SWAP_PROGRAM_ID,
        subs::{derive_controller_authority_pda, initialize_swap, pull_integration},
    };

    use super::*;

    #[tokio::test]

    async fn initialize_controller_and_token_swap() -> Result<(), Box<dyn std::error::Error>> {
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
            true, // can_manage_integrations
            true, // can_suspend_permissions
        )?;

        // Initialize a reserve for the USDS token
        let _usds_reserve_pk = initialize_reserve(
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

        // Initialize a reserve for the sUSDS token
        let _susds_reserve_pk = initialize_reserve(
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

        // Initialize a token swap for the pair
        let (usds_susds_swap_pk, usds_susds_lp_mint_pk) = initialize_swap(
            &mut svm,
            &authority,
            &authority,
            &usds_mint,
            &susds_mint,
            &NOVA_TOKEN_SWAP_PROGRAM_ID,
            1_000_000,
            1_000_000,
        )?;

        // Initialize an Integration

        let usds_susds_lp_vault_pk = get_associated_token_address_with_program_id(
            &controller_authority,
            &usds_susds_lp_mint_pk,
            &pinocchio_token::ID.into(),
        );

        let usdc_external_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // authority
            "USDS/sUSDS Token Swap",
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::SplTokenSwap(SplTokenSwapConfig {
                program: NOVA_TOKEN_SWAP_PROGRAM_ID,
                swap: usds_susds_swap_pk,
                mint_a: usds_mint,
                mint_b: susds_mint,
                lp_mint: usds_susds_lp_mint_pk,
                lp_token_account: usds_susds_lp_vault_pk,
                padding: [0; 32],
            }),
            &InitializeArgs::SplTokenSwap,
        )?;

        // Push the integration -- Add Liquidity to the swap pool
        push_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            &PushArgs::SplTokenSwap {
                amount_a: 100_000_000,
                amount_b: 120_000_000,
            },
        )
        .await?;

        // Pull the integration -- Withdraw liquidity from the swap pool
        pull_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            &PullArgs::SplTokenSwap {
                amount_a: 50_000_000,
                amount_b: 60_000_000,
            },
        )?;

        Ok(())
    }
}
