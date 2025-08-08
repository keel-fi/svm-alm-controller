mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, initialize_contoller, initialize_integration,
    initialize_reserve, manage_permission,
};
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};

#[cfg(test)]
mod tests {
    use solana_sdk::{clock::Clock, pubkey::Pubkey};
    use svm_alm_controller_client::generated::types::{
        InitializeArgs, KaminoConfig, PushArgs, ReserveStatus, UtilizationMarketConfig
    };

    use super::*;

      use crate::{
        helpers::constants::{
            KAMINO_LEND_PROGRAM_ID, KAMINO_MAIN_MARKET, KAMINO_USDC_RESERVE, KAMINO_USDC_RESERVE_FARM_COLLATERAL, KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, USDC_TOKEN_MINT_PUBKEY}, 
            subs::{
                derive_controller_authority_pda, derive_vanilla_obligation_address, edit_ata_amount, initialize_ata, push_integration, refresh_obligation, refresh_reserve, transfer_tokens
            }
    };

    #[tokio::test]
    async fn initialize_controller_and_kamino_integration() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let usdc_mint = USDC_TOKEN_MINT_PUBKEY;

        let authority = Keypair::new();
        
        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        // Create an ATA for the USDC account
        let _authority_usdc_ata = initialize_ata(
            &mut svm,
            &authority,
            &authority.pubkey(),
            &usdc_mint,
        )?;
        

        // Cheat to give the authority some USDC
        edit_ata_amount(
            &mut svm,
            &authority.pubkey(),
            &usdc_mint,
            1_000_000_000,
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
        let _usdc_reserve_pk = initialize_reserve(
            &mut svm,
            &controller_pk,
            &usdc_mint, // mint
            &authority, // payer
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
            &usdc_mint,
            &controller_authority,
            1_000_000_000,
        )?;

        let market = KAMINO_MAIN_MARKET;
        let reserve = KAMINO_USDC_RESERVE;
        let reserve_farm_collateral = KAMINO_USDC_RESERVE_FARM_COLLATERAL;
        let reserve_farm_debt = Pubkey::default();

        let obligation_id = 0;
        // Initialize a kamino main market USDC Integration
        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &market, 
            &KAMINO_LEND_PROGRAM_ID
        );

        let kamino_integration_pk = initialize_integration(
            &mut svm, 
            &controller_pk, 
            &authority, 
            &authority, 
            "Kamino main market/USDC", 
            IntegrationStatus::Active, 
            1_000_000_000_000, 
            1_000_000_000_000, 
            &IntegrationConfig::UtilizationMarket(
                UtilizationMarketConfig::KaminoConfig(KaminoConfig { 
                    market, 
                    reserve, 
                    reserve_farm_collateral,
                    reserve_farm_debt,
                    reserve_liquidity_mint: usdc_mint, 
                    obligation, 
                    obligation_id, 
                    padding: [0; 30] 
                })
            ), 
            &InitializeArgs::KaminoIntegration { obligation_id }
        )?;

        // advance time to avoid math overflow in kamino refresh calls
        let mut initial_clock = svm.get_sysvar::<Clock>();
        initial_clock.unix_timestamp = 1754682844;
        initial_clock.slot = 358753275;
        svm.set_sysvar::<Clock>(&initial_clock);

        // we refresh the reserve and the obligation
        refresh_reserve(
            &mut svm, 
            &authority, 
            &reserve, 
            &market, 
            &KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED
        )?;

        refresh_obligation(
            &mut svm, 
            &authority, 
            &market, 
            &obligation
        )?;

        // push the integration -- deposit reserve liquidity
        let _ = push_integration(
            &mut svm, 
            &controller_pk, 
            &kamino_integration_pk, 
            &authority, 
            &PushArgs::Kamino { amount: 1_000_000_000  } //1K
        )
        .await?;

        Ok(())
    }
}