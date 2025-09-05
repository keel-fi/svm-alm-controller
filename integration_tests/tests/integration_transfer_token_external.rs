mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, derive_controller_authority_pda, initialize_ata, initialize_contoller,
    initialize_integration, initialize_mint, initialize_reserve, manage_integration,
    manage_permission, manage_reserve, mint_tokens, push_integration,
};
use helpers::lite_svm_with_programs;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::generated::types::SplTokenExternalConfig;
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[tokio::test]
    #[test_case(spl_token::ID, None ; "SPL Token")]
    #[test_case(spl_token_2022::ID, None ; "Token2022")]
    #[test_case(spl_token_2022::ID, Some(100) ; "Token2022 TransferFee 100 bps")]

    async fn transfer_token_external_success(
        token_program: Pubkey,
        token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();
        let allocator = Keypair::new();
        let external = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &authority,
            &authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            token_transfer_fee,
        )?;

        let _authority_ata = initialize_ata(&mut svm, &authority, &authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &mint,
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
            true, // can_manage_reserves_and_integrations
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
            false, // can_manage_reserves_and_integrations
            false, // can_suspend_permissions
        )?;

        // Initialize a reserve for the token
        let _reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,      // mint
            &authority, // payer
            &authority, // authority
            ReserveStatus::Suspended,
            0, // rate_limit_slope
            0, // rate_limit_max_outflow
            &token_program,
        )?;

        // Update the reserve
        manage_reserve(
            &mut svm,
            &controller_pk,
            &mint,
            &authority,
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
        )?;

        // Initialize an External integration
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &token_program);
        let external_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // authority
            "DAO Treasury",
            IntegrationStatus::Suspended,
            0, // rate_limit_slope
            0, // rate_limit_max_outflow
            &IntegrationConfig::SplTokenExternal(SplTokenExternalConfig {
                program: token_program,
                mint: mint,
                recipient: external.pubkey(),
                token_account: external_ata,
                padding: [0; 96],
            }),
            &InitializeArgs::SplTokenExternal,
        )?;

        // Manage the integration
        manage_integration(
            &mut svm,
            &controller_pk,
            &external_integration_pk,
            &authority,
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
        )?;

        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &authority,
            &authority,
            &mint,
            &controller_authority,
            10_000_000,
        )?;

        // Push the integration
        push_integration(
            &mut svm,
            &controller_pk,
            &external_integration_pk,
            &authority,
            &PushArgs::SplTokenExternal { amount: 1_000_000 },
            false,
        )
        .await?;

        Ok(())
    }
}
