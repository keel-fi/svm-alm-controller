mod helpers;
mod subs;
use crate::subs::{
    airdrop_lamports, initialize_contoller,
    initialize_reserve, manage_permission,
};
use helpers::lite_svm_with_programs;
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};

#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use solana_sdk::{
        clock::Clock, 
        compute_budget::ComputeBudgetInstruction, 
        instruction::Instruction, 
        pubkey::Pubkey, 
        transaction::Transaction
    };
    use spl_associated_token_account_client::address::get_associated_token_address;
    use svm_alm_controller_client::{generated::types::{
        KaminoConfig, ReserveStatus, UtilizationMarketConfig
    }, 
    instructions::{
        initialize::kamino_init::get_kamino_init_ix, 
        pull::kamino_pull::get_kamino_pull_ix, 
        push::kamino_push::get_kamino_push_ix, 
        sync::kamino_sync::get_kamino_sync_ix
    }};

    use super::*;

    use crate::{
        helpers::{constants::{
            BONK_MINT, 
            KAMINO_FARMS_PROGRAM_ID, 
            KAMINO_LEND_PROGRAM_ID, 
            KAMINO_MAIN_MARKET, 
            KAMINO_REFERRER_METADATA, 
            KAMINO_USDC_RESERVE, 
            KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT, 
            KAMINO_USDC_RESERVE_BONK_VAULT, 
            KAMINO_USDC_RESERVE_COLLATERAL_MINT, 
            KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY, 
            KAMINO_USDC_RESERVE_FARM_COLLATERAL, 
            KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, 
            KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY, 
            KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, 
            USDC_TOKEN_MINT_PUBKEY
        }, get_account_data_from_json}, 
        subs::{
            derive_controller_authority_pda, 
            derive_vanilla_obligation_address, 
            edit_ata_amount, 
            initialize_ata, 
            refresh_obligation, 
            refresh_reserve, 
            transfer_tokens
        }
    };

    #[tokio::test]
    async fn initialize_controller_and_kamino_integration() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        set_kamino_accounts(&mut svm);
        let usdc_mint = USDC_TOKEN_MINT_PUBKEY;

        let authority = Keypair::new();
        let authority_2 = Keypair::new();
        
        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &authority_2.pubkey(), 1_000_000_000)?;

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

        let kamino_config = KaminoConfig { 
            market, 
            reserve, 
            reserve_farm_collateral,
            reserve_farm_debt,
            reserve_liquidity_mint: usdc_mint, 
            obligation, 
            obligation_id, 
            padding: [0; 30] 
        };

        let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);

        let (
            init_kamino_ix, 
            kamino_integration_pk
        ) = get_kamino_init_ix(
            &controller_pk, 
            &authority.pubkey(), 
            &authority.pubkey(), 
            "Kamino main market/USDC",
            IntegrationStatus::Active,
            1_000_000_000_000, 
            1_000_000_000_000, 
            &IntegrationConfig::UtilizationMarket(
                UtilizationMarketConfig::KaminoConfig(kamino_config.clone())
            ), 
            svm.get_sysvar::<Clock>().slot, 
            obligation_id
        );
        build_and_send_tx(
            &mut svm,
            &[cu_limit_ix.clone(), init_kamino_ix],
            &authority,
            &authority
        );

        // advance time to avoid math overflow in kamino refresh calls
        let mut initial_clock = svm.get_sysvar::<Clock>();
        initial_clock.unix_timestamp = 1754682844;
        initial_clock.slot = 358754275;
        svm.set_sysvar::<Clock>(&initial_clock);

        // we refresh the reserve and the obligation
        refresh_reserve(
            &mut svm, 
            &authority, 
            &reserve, 
            &market, 
            &KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED,
        )?;

        refresh_obligation(
            &mut svm, 
            &authority, 
            &market, 
            &obligation,
            None
        )?;

        // push the integration -- deposit reserve liquidity

        let push_ix = get_kamino_push_ix(
            &controller_pk, 
            &kamino_integration_pk, 
            &authority.pubkey(), 
            &kamino_config, 
            1_000_000_000
        );
        build_and_send_tx(
            &mut svm,
            &[cu_limit_ix.clone(), push_ix],
            &authority,
            &authority
        );

        // advance time again for pull
        let mut post_push_clock = svm.get_sysvar::<Clock>();
        post_push_clock.unix_timestamp = 1754948368;
        post_push_clock.slot = 359424320;
        svm.set_sysvar::<Clock>(&post_push_clock);

        // we refresh the reserve and the obligation
        refresh_reserve(
            &mut svm, 
            &authority_2, 
            &reserve, 
            &market, 
            &KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED,
        )?;

        refresh_obligation(
            &mut svm, 
            &authority_2, 
            &market, 
            &obligation,
            Some(&reserve)
        )?;

        let rewards_ata = get_associated_token_address(
            &controller_authority, 
            &BONK_MINT
        );

        // let rewards_ata = Pubkey::default();

        // sync the integration with bonk as reward args

        let sync_ix = get_kamino_sync_ix(
            &controller_pk, 
            &kamino_integration_pk, 
            &authority.pubkey(), 
            &kamino_config, 
            &BONK_MINT, 
            &KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, 
            &rewards_ata, 
            &KAMINO_FARMS_PROGRAM_ID, 
            &spl_token::ID
        );
        build_and_send_tx(
            &mut svm,
            &[cu_limit_ix.clone(), sync_ix],
            &authority,
            &authority
        );

        // pull
        let pull_ix = get_kamino_pull_ix(
            &controller_pk, 
            &kamino_integration_pk, 
            &authority.pubkey(), 
            &kamino_config, 
            900_000_000
        );
        build_and_send_tx(
            &mut svm,
            &[cu_limit_ix.clone(), pull_ix],
            &authority,
            &authority
        );

        Ok(())
    }

    fn build_and_send_tx(
        svm: &mut LiteSVM,
        ixs: &[Instruction],
        authority: &Keypair,
        payer: &Keypair
    ) {
        let tx = Transaction::new_signed_with_payer(
            ixs, 
            Some(&payer.pubkey()), 
            &[&authority, payer], 
            svm.latest_blockhash()
        );
        let tx_result = svm.send_transaction(tx);
        if tx_result.is_err() {
            println!("{:#?}", tx_result.unwrap().logs);
        } else {
            assert!(tx_result.is_ok(), "Transaction failed to execute");
        }
    }

    fn set_kamino_accounts(svm: &mut LiteSVM) {
        let kamino_main_market_account = get_account_data_from_json("./fixtures/kamino_main_market.json");
        svm.set_account(KAMINO_MAIN_MARKET, kamino_main_market_account)
            .unwrap();
        let kamino_usdc_reserve = get_account_data_from_json("./fixtures/kamino_usdc_reserve.json");
        svm.set_account(KAMINO_USDC_RESERVE, kamino_usdc_reserve)
            .unwrap();
        let kamino_usdc_reserve_farm_collateral = get_account_data_from_json("./fixtures/usdc_reserve_farm_collateral.json");
        svm.set_account(KAMINO_USDC_RESERVE_FARM_COLLATERAL, kamino_usdc_reserve_farm_collateral)
            .unwrap();
        let kamino_referrer_user_metadata = get_account_data_from_json("./fixtures/kamino_referrer_metadata.json");
        svm.set_account(KAMINO_REFERRER_METADATA, kamino_referrer_user_metadata)
            .unwrap();
        let kamino_usdc_reserve_liquidity_supply = get_account_data_from_json("./fixtures/kamino_usdc_reserve_liquidity_supply.json");
        svm.set_account(KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY, kamino_usdc_reserve_liquidity_supply)
            .unwrap();
        let kamino_usdc_reserve_collateral_mint = get_account_data_from_json("./fixtures/kamino_usdc_reserve_collateral_mint.json");
        svm.set_account(KAMINO_USDC_RESERVE_COLLATERAL_MINT, kamino_usdc_reserve_collateral_mint)
            .unwrap();
        let kamino_usdc_reserve_collateral_supply = get_account_data_from_json("./fixtures/kamino_usdc_reserve_collateral_supply.json");
        svm.set_account(KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY, kamino_usdc_reserve_collateral_supply)
            .unwrap();
        let kamino_usdc_reserve_scope_config_price_feed = get_account_data_from_json("./fixtures/kamino_usdc_reserve_scope_config_price_feed.json");
        svm.set_account(KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED, kamino_usdc_reserve_scope_config_price_feed)
            .unwrap();
        let kamino_usdc_reserve_farm_global_config = get_account_data_from_json("./fixtures/kamino_farm_global_config.json");
        svm.set_account(KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG, kamino_usdc_reserve_farm_global_config)
            .unwrap();
        let bonk_mint = get_account_data_from_json("./fixtures/bonk_mint.json");
        svm.set_account(BONK_MINT, bonk_mint)
            .unwrap();
        let bonk_reward_vault = get_account_data_from_json("./fixtures/usdc_reserve_bonk_vault.json");
        svm.set_account(KAMINO_USDC_RESERVE_BONK_VAULT, bonk_reward_vault)
            .unwrap();
        let bonk_treasury_vaut = get_account_data_from_json("./fixtures/usdc_reserve_bonk_treasury_vault.json");
        svm.set_account(KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT, bonk_treasury_vaut)
            .unwrap();
    }
}