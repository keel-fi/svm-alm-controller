mod helpers;
mod subs;
use crate::subs::{
    derive_controller_authority_pda, initialize_ata, initialize_integration, initialize_mint,
    initialize_reserve, manage_integration, manage_permission, manage_reserve, mint_tokens,
    push_integration,
};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::generated::types::SplTokenExternalConfig;
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};
use svm_alm_controller_client::generated::types::{
    IntegrationConfig, IntegrationStatus, PermissionStatus,
};

#[cfg(test)]
mod tests {
    use crate::{
        helpers::{setup_test_controller, TestContext},
        subs::airdrop_lamports,
    };

    use super::*;
    use solana_sdk::{instruction::InstructionError, transaction::TransactionError};
    use test_case::test_case;

    #[tokio::test]
    #[test_case(spl_token::ID, None ; "SPL Token")]
    #[test_case(spl_token_2022::ID, None ; "Token2022")]
    #[test_case(spl_token_2022::ID, Some(100) ; "Token2022 TransferFee 100 bps")]

    async fn transfer_token_external_success(
        token_program: Pubkey,
        token_transfer_fee: Option<u16>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let external = Keypair::new();

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            token_transfer_fee,
        )?;

        let _authority_ata =
            initialize_ata(&mut svm, &super_authority, &super_authority.pubkey(), &mint)?;

        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &super_authority.pubkey(),
            1_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let _reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
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
            &super_authority,
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
            &super_authority, // payer
            &super_authority, // authority
            "DAO Treasury",
            IntegrationStatus::Suspended,
            0,     // rate_limit_slope
            0,     // rate_limit_max_outflow
            false, // permit_liquidation
            &IntegrationConfig::SplTokenExternal(SplTokenExternalConfig {
                program: token_program,
                mint: mint,
                recipient: external.pubkey(),
                token_account: external_ata,
                padding: [0; 96],
            }),
            &InitializeArgs::SplTokenExternal,
            false,
        ).map_err(|e| e.err.to_string())?;

        // Manage the integration
        manage_integration(
            &mut svm,
            &controller_pk,
            &external_integration_pk,
            &super_authority,
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
        )?;

        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &controller_authority,
            10_000_000,
        )?;

        // Push the integration
        let (tx_result, _) = push_integration(
            &mut svm,
            &controller_pk,
            &external_integration_pk,
            &super_authority,
            &PushArgs::SplTokenExternal { amount: 1_000_000 },
            false,
        )
        .await?;
        tx_result.unwrap();

        Ok(())
    }

    #[test_case(true, false, false, false, false, false, false, false, false, false, false; "can_manage_permissions fails")]
    #[test_case(false, true, false, false, false, false, false, false, false, false, true; "can_invoke_external_transfer passes")]
    #[test_case(false, false, true, false, false, false, false, false, false, false, false; "can_execute_swap fails")]
    #[test_case(false, false, false, true, false, false, false, false, false, false, false; "can_reallocate fails")]
    #[test_case(false, false, false, false, true, false, false, false, false, false, false; "can_freeze_controller fails")]
    #[test_case(false, false, false, false, false, true, false, false, false, false, false; "can_unfreeze_controller fails")]
    #[test_case(false, false, false, false, false, false, true, false, false, false, false; "can_manage_reserves_and_integrations fails")]
    #[test_case(false, false, false, false, false, false, false, true, false, false, false; "can_suspend_permissions fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, false, false; "can_liquidate w/o permit_liquidation fails")]
    #[test_case(false, false, false, false, false, false, false, false, true, true, false; "can_liquidate w/ permit_liquidation fails")]
    #[tokio::test]
    async fn spl_token_external_permissions(
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
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let external = Keypair::new();

        // Setup Token Program and Controller state

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let _reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Initialize an External integration
        let external_ata =
            get_associated_token_address_with_program_id(&external.pubkey(), &mint, &spl_token::ID);
        let external_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // authority
            "DAO Treasury",
            IntegrationStatus::Active,
            1_000_000_000_000,  // rate_limit_slope
            1_000_000_000_000,  // rate_limit_max_outflow
            permit_liquidation, // permit_liquidation
            &IntegrationConfig::SplTokenExternal(SplTokenExternalConfig {
                program: spl_token::ID,
                mint: mint,
                recipient: external.pubkey(),
                token_account: external_ata,
                padding: [0; 96],
            }),
            &InitializeArgs::SplTokenExternal,
        )?;

        // Transfer funds directly to the controller's vault
        mint_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &mint,
            &controller_authority,
            10_000_000,
        )?;

        // Setup Permission state and invoke push
        let push_authority = Keypair::new();
        airdrop_lamports(&mut svm, &push_authority.pubkey(), 1_000_000_000)?;
        // Update the authority to have permissions
        manage_permission(
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
            &external_integration_pk,
            &push_authority,
            &PushArgs::SplTokenExternal { amount: 1_000_000 },
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
}
