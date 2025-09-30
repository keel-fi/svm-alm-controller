mod helpers;
mod subs;
use crate::helpers::constants::USDC_TOKEN_MINT_PUBKEY;
use crate::subs::{
    derive_controller_authority_pda, edit_ata_amount, initialize_ata, initialize_reserve,
    manage_permission, push_integration, transfer_tokens,
};
use helpers::{
    cctp::evm_address_to_solana_pubkey, constants::CCTP_REMOTE_DOMAIN_ETH, setup_test_controller,
    TestContext,
};
use solana_sdk::signer::Signer;
use svm_alm_controller_client::generated::types::{IntegrationStatus, PermissionStatus};
use svm_alm_controller_client::generated::types::{PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {

    use solana_sdk::{
        instruction::InstructionError,
        signature::Keypair,
        transaction::{Transaction, TransactionError},
    };
    use svm_alm_controller_client::create_cctp_bridge_initialize_integration_instruction;
    use test_case::test_case;

    use crate::subs::airdrop_lamports;

    use super::*;

    #[tokio::test]
    async fn cctp_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            false,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

        // Push the integration -- i.e. bridge using CCTP
        let (tx_res, _) = push_integration(
            &mut svm,
            &controller_pk,
            &cctp_usdc_eth_bridge_integration_pk,
            &super_authority,
            &PushArgs::CctpBridge { amount: 1_000_000 },
            false,
        )
        .await?;

        tx_res.unwrap();

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
    async fn cctp_permissions(
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

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Initialize a reserve for the token
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &super_authority,        // payer
            &super_authority,        // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &super_authority,
            &super_authority,
            &USDC_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let init_integration_ix = create_cctp_bridge_initialize_integration_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
            permit_liquidation,
            &USDC_TOKEN_MINT_PUBKEY,
            &destination_address,
            CCTP_REMOTE_DOMAIN_ETH,
        );
        // Integration is at index 5 in the IX
        let cctp_usdc_eth_bridge_integration_pk = init_integration_ix.accounts[5].pubkey;
        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&super_authority.pubkey()),
            &[&super_authority],
            svm.latest_blockhash(),
        ))
        .map_err(|e| e.err.to_string())?;

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

        // Push the integration -- i.e. bridge using CCTP
        let (tx_res, _) = push_integration(
            &mut svm,
            &controller_pk,
            &cctp_usdc_eth_bridge_integration_pk,
            &push_authority,
            &PushArgs::CctpBridge { amount: 1_000_000 },
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
