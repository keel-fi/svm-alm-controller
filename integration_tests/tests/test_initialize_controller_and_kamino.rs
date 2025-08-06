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
    use solana_sdk::pubkey;
    use svm_alm_controller_client::generated::types::{InitializeArgs, KaminoConfig, ReserveStatus, UtilizationMarketConfig};

    use super::*;

      use crate::{
        helpers::constants::{KAMINO_LEND_PROGRAM_ID, KAMINO_MAIN_MARKET, KAMINO_USDC_RESERVE, KAMINO_USDC_RESERVE_FARM_COLLATERAL}, subs::{derive_controller_authority_pda, derive_vanilla_obligation_address}
    };

    #[tokio::test]
    async fn initialize_controller_and_kamino() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let usdc_mint = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

        let authority = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

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


        let market = KAMINO_MAIN_MARKET;
        let reserve = KAMINO_USDC_RESERVE;
        let reserve_farm = KAMINO_USDC_RESERVE_FARM_COLLATERAL;

        let obligation_id = 0;
        // Initialize a kamino main market USDC Integration
        let obligation = derive_vanilla_obligation_address(
            obligation_id, 
            &controller_authority, 
            &market, 
            &KAMINO_LEND_PROGRAM_ID
        );

        let _kamino_integration = initialize_integration(
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
                    reserve_farm,
                    token_mint: usdc_mint,
                    obligation,
                    obligation_id
                })
            ), 
            &InitializeArgs::KaminoIntegration { obligation_id }
        )?;



        Ok(())
    }
}