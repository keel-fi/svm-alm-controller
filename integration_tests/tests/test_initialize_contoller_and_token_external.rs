mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, derive_controller_authority_pda, initialize_ata, initialize_contoller,
    initialize_integration, initialize_mint, initialize_reserve, manage_integration,
    manage_permission, manage_reserve, mint_tokens, push_integration,
};
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::generated::types::SplTokenExternalConfig;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]

    async fn initialize_controller_and_token_external_success(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();
        let allocator = Keypair::new();
        let external = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        // Initialize a mint
        let usdc_mint = initialize_mint(
            &mut svm,
            &authority,
            &authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
        )?;

        let _authority_usdc_ata =
            initialize_ata(&mut svm, &authority, &authority.pubkey(), &usdc_mint)?;

        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &usdc_mint,
            &authority.pubkey(),
            1_000_000,
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

        // Create a new permission for an allocator
        let _allocator_permission_pk = manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &allocator.pubkey(), // subject authority
            PermissionStatus::Active,
            true,  // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            true,  // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_integrations
            false, // can_suspend_permissions
        )?;

        // Initialize a reserve for the token
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &usdc_mint, // mint
            &authority, // payer
            &authority, // authority
            ReserveStatus::Suspended,
            0, // rate_limit_slope
            0, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Update the reserve
        manage_reserve(
            &mut svm,
            &controller_pk,
            &usdc_mint,
            &authority,
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
        )?;

        // Initialize an External integration
        let external_usdc_ata = get_associated_token_address_with_program_id(
            &external.pubkey(),
            &usdc_mint,
            &pinocchio_token::ID.into(),
        );
        let usdc_external_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // authority
            "USDC DAO Treasury",
            IntegrationStatus::Suspended,
            0, // rate_limit_slope
            0, // rate_limit_max_outflow
            &IntegrationConfig::SplTokenExternal(SplTokenExternalConfig {
                program: pinocchio_token::ID.into(),
                mint: usdc_mint,
                recipient: external.pubkey(),
                token_account: external_usdc_ata,
                padding: [0; 96],
            }),
            &InitializeArgs::SplTokenExternal,
        )?;

        // Manage the integration
        manage_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
        )?;

        // TODO: Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &usdc_mint,
            &controller_authority,
            10_000_000,
        )?;

        // Push the integration
        push_integration(
            &mut svm,
            &controller_pk,
            &usdc_external_integration_pk,
            &authority,
            &PushArgs::SplTokenExternal { amount: 1_000_000 },
        )
        .await?;

        Ok(())
    }
}
