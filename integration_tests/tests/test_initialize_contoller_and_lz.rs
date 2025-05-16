mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, initialize_ata, initialize_contoller, initialize_integration,
    initialize_reserve, manage_permission, push_integration,
};
use crate::{
    helpers::constants::USDS_TOKEN_MINT_PUBKEY,
    subs::{edit_ata_amount, transfer_tokens},
};
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {

    use svm_alm_controller_client::generated::types::LzBridgeConfig;

    use crate::helpers::{
        cctp::evm_address_to_solana_pubkey,
        constants::{
            LZ_DESTINATION_DOMAIN_EID, LZ_USDS_OFT_PROGRAM_ID, LZ_USDS_OFT_STORE_PUBKEY,
            LZ_USDS_PEER_CONFIG_PUBKEY,
        },
    };

    use super::*;

    #[test_log::test]
    fn initialize_controller_and_lz() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        // Create an ATA for the USDC account
        let _authority_usds_ata = initialize_ata(
            &mut svm,
            &authority,
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            321u16, // Id
        )?;

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
        )?;

        // Initialize a reserve for the token
        let _usds_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDS_TOKEN_MINT_PUBKEY, // mint
            &authority,              // payer
            &authority,              // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm,
            &authority,
            &authority,
            &USDS_TOKEN_MINT_PUBKEY,
            &controller_pk,
            500_000_000,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        // // Initialize an integration
        let lz_usds_eth_bridge_integration_pk = initialize_integration(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // authority
            "ETH USDS LZ Bridge",
            IntegrationStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::LzBridge(LzBridgeConfig {
                program: LZ_USDS_OFT_PROGRAM_ID,
                mint: USDS_TOKEN_MINT_PUBKEY,
                destination_address: destination_address,
                destination_eid: LZ_DESTINATION_DOMAIN_EID,
                oft_store: LZ_USDS_OFT_STORE_PUBKEY,
                peer_config: LZ_USDS_PEER_CONFIG_PUBKEY,
                padding: [0; 28],
            }),
            &InitializeArgs::LzBridge {
                desination_address: destination_address,
                destination_eid: LZ_DESTINATION_DOMAIN_EID,
            },
        )?;

        // Push the integration -- i.e. bridge using LZ OFT
        push_integration(
            &mut svm,
            &controller_pk,
            &lz_usds_eth_bridge_integration_pk,
            &authority,
            &PushArgs::LzBridge { amount: 1_000_000 },
        )?;

        Ok(())
    }
}
