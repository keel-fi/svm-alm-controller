mod helpers;
mod subs;
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::types::{ControllerStatus,PermissionStatus, IntegrationConfig, IntegrationStatus};
use svm_alm_controller_client::types::SplTokenExternalConfig;
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller_client::types::{InitializeArgs, PushArgs, ReserveStatus};
use crate::subs::{airdrop_lamports, initialize_mint, initialize_ata, initialize_contoller, initialize_integration, initialize_reserve, manage_integration, manage_permission, manage_reserve, mint_tokens, push_integration};
use crate::helpers::constants::USDC_TOKEN_MINT_PUBKEY;


#[cfg(test)]
mod tests {
  

    use svm_alm_controller_client::types::{CctpBridgeConfig, DepositForBurnArgs};

    use crate::{helpers::{cctp::{evm_address_to_solana_pubkey, CctpDepositForBurnPdas}, constants::{CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID, CCTP_REMOTE_DOMAIN_ETH, CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID}}, subs::{edit_ata_amount, transfer_tokens}};

    use super::*;

    #[test_log::test]
    fn initialize_controller_and_cctp() -> Result<(), Box<dyn std::error::Error>> {

        let mut svm = lite_svm_with_programs();
    
        let authority = Keypair::new();
  
        // Airdrop to payer
        airdrop_lamports(
            &mut svm, 
            &authority.pubkey(), 
            1_000_000_000
        )?;

        // Create an ATA for the USDC account 
        let authority_usdc_ata = initialize_ata(
            &mut svm, 
            &authority, 
            &authority.pubkey(), 
            &USDC_TOKEN_MINT_PUBKEY
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &authority.pubkey(), 
            &USDC_TOKEN_MINT_PUBKEY, 
            1_000_000_000
        )?;
        

        let (controller_pk, authority_permission_pk) = initialize_contoller(
            &mut svm, 
            &authority, 
            &authority, 
            ControllerStatus::Active, 
            321u16 // Id
        )?;

        // Update the authority to have all permissions
        let _ = manage_permission(
            &mut svm,
            &controller_pk, 
            &authority,  // payer
            &authority, // calling authority
            &authority.pubkey(),  // subject authority
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
        let usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk, 
            &USDC_TOKEN_MINT_PUBKEY, // mint
            &authority,  // payer
            &authority, // authority
            ReserveStatus::Active, 
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
        )?;

      
        // Transfer funds into the reserve
        transfer_tokens(
            &mut svm, 
            &authority, 
            &authority, 
            &USDC_TOKEN_MINT_PUBKEY, 
            &controller_pk, 
            500_000_000
        )?;


        // Serialize the destination address appropriately
        let evm_address = "0x3BF0730133daa6398F3bcDBaf5395A9C86116642";
        let destination_address= evm_address_to_solana_pubkey(evm_address);

        
        // Initialize an External integration
        let cctp_usdc_eth_bridge_integration_pk = initialize_integration(
            &mut svm, 
            &controller_pk, 
            &authority, // payer
            &authority, // authority
            "ETH USDC CCTP Bridge",
            IntegrationStatus::Active, 
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &IntegrationConfig::CctpBridge(
                CctpBridgeConfig {
                    cctp_token_messenger_minter: CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
                    cctp_message_transmitter: CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID,
                    mint: USDC_TOKEN_MINT_PUBKEY,
                    destination_address: destination_address,
                    destination_domain: CCTP_REMOTE_DOMAIN_ETH,
                    padding: [0;60],
                }
            ),
            &InitializeArgs::CctpBridge { 
                desination_address: destination_address,
                desination_domain: CCTP_REMOTE_DOMAIN_ETH 
            }
        )?;
        
   
        // Push the integration -- i.e. bridge using CCTP
        push_integration(
            &mut svm,
            &controller_pk, 
            &cctp_usdc_eth_bridge_integration_pk,
            &authority,
            &PushArgs::CctpBridge {
                amount: 1_000_000
            }
        )?;


        Ok(())
    }


}