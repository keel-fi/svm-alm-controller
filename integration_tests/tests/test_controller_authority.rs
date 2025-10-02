mod helpers;
mod subs;

#[cfg(test)]
mod tests {
     use super::*;
    use litesvm::LiteSVM;
    use solana_keccak_hasher::hash;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction}, 
        pubkey::Pubkey, signature::Keypair, 
        signer::Signer, 
        system_program, 
        transaction::Transaction
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use subs::controller::initialize_contoller;
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        derive_permission_pda, 
        generated::{
            instructions::{
                InitializeIntegrationBuilder, InitializeOracleBuilder, 
                InitializeReserveBuilder, ManageControllerBuilder, 
                ManageIntegrationBuilder, ManagePermissionBuilder, 
                ManageReserveBuilder, SyncReserveBuilder, UpdateOracleBuilder
            }, 
            types::{
                ControllerStatus, FeedArgs, 
                InitializeArgs, IntegrationConfig, 
                IntegrationStatus, IntegrationType, 
                PermissionStatus, ReserveStatus, SplTokenExternalConfig
            }
        }
    };
    use crate::{
        helpers::{
            assert::assert_custom_error, lite_svm_with_programs
        }, 
        subs::{
            airdrop_lamports, derive_controller_authority_pda, 
            derive_integration_pda, derive_reserve_pda, initialize_mint, 
            manage_permission, 
            oracle::{derive_oracle_pda, set_price_feed}
        }
    };

    fn get_init_oracle_ix(
        svm: &mut LiteSVM,
        payer_and_authority: &Keypair,
        controller_pk: &Pubkey,
        fake_controller_authority: Option<&Pubkey>
    ) -> Result<(Instruction, Pubkey, Pubkey), Box<dyn std::error::Error>> {
        let nonce = Pubkey::new_unique();
        let price_feed = Pubkey::new_unique();
        let oracle_pda = derive_oracle_pda(&nonce);
        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let controller_authority_to_use = match fake_controller_authority {
            Some(fake) => fake,
            None => &controller_authority
        };

        let update_slot = 1000_000;
        let update_price = 1_000_000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(svm, &price_feed, update_price)?;

        let init_ixn = InitializeOracleBuilder::new()
            .controller(*controller_pk)
            .controller_authority(*controller_authority_to_use)
            .authority(payer_and_authority.pubkey())
            .oracle(oracle_pda)
            .price_feed(price_feed)
            .system_program(system_program::ID)
            .payer(payer_and_authority.pubkey())
            .oracle_type(0)
            .nonce(nonce)
            .base_mint(Pubkey::new_unique())
            .quote_mint(Pubkey::new_unique())
            .instruction();

        Ok((init_ixn, oracle_pda, price_feed))
    }

    fn get_init_reserve_ix(
        svm: &mut LiteSVM,
        payer_and_authority: &Keypair,
        controller_pk: &Pubkey,
        permission_pda: &Pubkey,
        fake_controller_authority: Option<&Pubkey>
    ) -> (Instruction, Pubkey, Pubkey) {

        let mint = initialize_mint(
            svm,
            &payer_and_authority,
            &payer_and_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )
        .unwrap();

        let reserve_pda = derive_reserve_pda(&controller_pk, &mint);

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let vault =
            get_associated_token_address_with_program_id(&controller_authority, &mint, &spl_token::ID);

        let controller_authority_to_use = match fake_controller_authority {
            Some(fake) => fake,
            None => &controller_authority
        };

        let ixn = InitializeReserveBuilder::new()
            .status(ReserveStatus::Active)
            .rate_limit_slope(1000000)
            .rate_limit_max_outflow(1000000)
            .payer(payer_and_authority.pubkey())
            .controller(*controller_pk)
            .controller_authority(*controller_authority_to_use)
            .authority(payer_and_authority.pubkey())
            .permission(*permission_pda)
            .reserve(reserve_pda)
            .mint(mint)
            .vault(vault)
            .token_program(spl_token::ID)
            .associated_token_program(pinocchio_associated_token_account::ID.into())
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .system_program(system_program::ID)
            .instruction();

        (ixn, reserve_pda, vault)
    }

    fn get_init_token_external_integration_ix(
        svm: &mut LiteSVM,
        payer_and_authority: &Keypair,
        controller_pk: &Pubkey,
        permission_pda: &Pubkey,
        controller_authority: &Pubkey
    ) -> (Instruction, Pubkey) {
        let permit_liquidation = true;
        // Initialize a mint
        let mint = initialize_mint(
            svm,
            payer_and_authority,
            &payer_and_authority.pubkey(),
            None,
            6,
            None,
            &spl_token::ID,
            None,
        )
        .unwrap();
        let external = Pubkey::new_unique();
        let description = "DAO Treasury".to_string();
        let external_ata = spl_associated_token_account_client::address::get_associated_token_address_with_program_id(
            &external,
            &mint,
            &spl_token::ID,
        );

        let config = IntegrationConfig::SplTokenExternal(SplTokenExternalConfig {
            program: spl_token::ID,
            mint: mint,
            recipient: external,
            token_account: external_ata,
            padding: [0u8; 96],
        });

        let inner_args = InitializeArgs::SplTokenExternal;

        let hash = hash(borsh::to_vec(&config).unwrap().as_ref()).to_bytes();
        let integration_pda = derive_integration_pda(&controller_pk, &hash);

        let description_bytes = description.as_bytes();
        let mut description_encoding: [u8; 32] = [0; 32];
        description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

        let remaining_accounts = [
            AccountMeta {
                pubkey: mint,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: external,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: external_ata,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: spl_token::ID,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_associated_token_account_client::program::ID,
                is_signer: false,
                is_writable: false,
            },
        ];

        let ix = InitializeIntegrationBuilder::new()
            .integration_type(IntegrationType::SplTokenExternal)
            .status(IntegrationStatus::Active)
            .description(description_encoding)
            .rate_limit_slope(1000000)
            .rate_limit_max_outflow(1000000)
            .permit_liquidation(permit_liquidation)
            .inner_args(inner_args.clone())
            .payer(payer_and_authority.pubkey())
            .controller(*controller_pk)
            .controller_authority(*controller_authority)
            .authority(payer_and_authority.pubkey())
            .permission(*permission_pda)
            .integration(integration_pda)
            .add_remaining_accounts(&remaining_accounts)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .system_program(system_program::ID)
            .instruction();

        (ix, integration_pda)
    }

    fn setup_test_env() -> Result<(LiteSVM, Keypair, Pubkey, Pubkey), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let payer_and_authority = Keypair::new();
        airdrop_lamports(&mut svm, &payer_and_authority.pubkey(), 10_000_000_000)?;

        let (controller_pk, permission_pda) = initialize_contoller(
            &mut svm,
            &payer_and_authority,
            &payer_and_authority,
            ControllerStatus::Active,
            0
        )?;

        // Update the authority to have all permissions
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &payer_and_authority,          // payer
            &payer_and_authority,          // calling authority
            &payer_and_authority.pubkey(), // subject authority
            PermissionStatus::Active,
            true, // can_execute_swap,
            true, // can_manage_permissions,
            true, // can_invoke_external_transfer,
            true, // can_reallocate,
            true, // can_freeze,
            true, // can_unfreeze,
            true, // can_manage_reserves_and_integrations
            true, // can_suspend_permissions
            true, // can_liquidate
        )?;

        Ok((svm, payer_and_authority, controller_pk, permission_pda))
    }

    #[test]
    fn test_wrong_controller_authority_manage_controller_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            permission_pda
        ) = setup_test_env()?;

        // fake controller authority should throw InvalidControllerAuthority error
        let fake_controller_authority = Pubkey::new_unique();

        let ixn = ManageControllerBuilder::new()
            .status(ControllerStatus::Active)
            .controller(controller_pk)
            .controller_authority(fake_controller_authority)
            .authority(payer_and_authority.pubkey())
            .permission(permission_pda)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .instruction();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_init_integration_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            permission_pda
        ) = setup_test_env()?;


        let fake_controller_authority = Pubkey::new_unique();

        let (ix, _) = get_init_token_external_integration_ix(
            &mut svm,
            &payer_and_authority,
            &controller_pk,
            &permission_pda,
            &fake_controller_authority
        );

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_manage_integration_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            permission_pda
        ) = setup_test_env()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);
        let (init_integration_ix, integration_pk) = get_init_token_external_integration_ix(
            &mut svm, 
            &payer_and_authority, 
            &controller_pk, 
            &permission_pda, 
            &controller_authority
        );

        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_integration_ix],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        )).unwrap();

        let fake_controller_authority = Pubkey::new_unique();

        let description = "DAO Treasury".to_string();
        let description_bytes = description.as_bytes();
        let mut description_encoding: [u8; 32] = [0; 32];
        description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

        let manage_integration_ix = ManageIntegrationBuilder::new()
            .status(IntegrationStatus::Active)
            .rate_limit_slope(1000000)
            .rate_limit_max_outflow(1000000)
            .description(description_encoding)
            .controller(controller_pk)
            .controller_authority(fake_controller_authority)
            .authority(payer_and_authority.pubkey())
            .permission(permission_pda)
            .integration(integration_pk)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .instruction();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[manage_integration_ix],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_manage_permission_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            permission_pda
        ) = setup_test_env()?;


        let subject_authority = Keypair::new();
        let subject_permission_pda = derive_permission_pda(&controller_pk, &subject_authority.pubkey());


        let fake_controller_authority = Pubkey::new_unique();

        let ixn = ManagePermissionBuilder::new()
            .status(PermissionStatus::Active)
            .can_execute_swap(false)
            .can_manage_permissions(false)
            .can_invoke_external_transfer(false)
            .can_reallocate(false)
            .can_freeze_controller(false)
            .can_unfreeze_controller(false)
            .can_manage_reserves_and_integrations(false)
            .can_suspend_permissions(false)
            .can_liquidate(false)
            .payer(payer_and_authority.pubkey())
            .controller(controller_pk)
            .controller_authority(fake_controller_authority)
            .super_authority(payer_and_authority.pubkey())
            .super_permission(permission_pda)
            .authority(subject_authority.pubkey())
            .permission(subject_permission_pda)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .system_program(system_program::ID)
            .instruction();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_init_reserve_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            permission_pda
        ) = setup_test_env()?;
    
        let fake_controller_authority = Pubkey::new_unique();

        let (ixn, _, _) = get_init_reserve_ix(
            &mut svm, 
            &payer_and_authority, 
            &controller_pk, 
            &permission_pda, 
            Some(&fake_controller_authority)
        );

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_manage_reserve_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            permission_pda
        ) = setup_test_env()?;

        let (init_ixn, reserve_pda, _) = get_init_reserve_ix(
            &mut svm, 
            &payer_and_authority, 
            &controller_pk, 
            &permission_pda, 
            None
        );

        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        )).unwrap();

        let fake_controller_authority = Pubkey::new_unique();
        let manage_ixn = ManageReserveBuilder::new()
            .status(ReserveStatus::Active)
            .rate_limit_slope(100)
            .rate_limit_max_outflow(100)
            .controller(controller_pk)
            .controller_authority(fake_controller_authority)
            .authority(payer_and_authority.pubkey())
            .permission(permission_pda)
            .reserve(reserve_pda)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .instruction();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[manage_ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_sync_reserve_fails () -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            permission_pda
        ) = setup_test_env()?;

        let (init_ixn, reserve_pda, vault) = get_init_reserve_ix(
            &mut svm, 
            &payer_and_authority, 
            &controller_pk, 
            &permission_pda, 
            None
        );

        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        )).unwrap();

        let fake_controller_authority = Pubkey::new_unique();
        let sync_ixn = SyncReserveBuilder::new()
            .controller(controller_pk)
            .controller_authority(fake_controller_authority)
            .reserve(reserve_pda)
            .vault(vault)
            .instruction();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[sync_ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_init_oracle_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            _permission_pda
        ) = setup_test_env()?;
        let fake_controller_authority = Pubkey::new_unique();

        let (init_ixn, _, _) = get_init_oracle_ix(
            &mut svm,
            &payer_and_authority, 
            &controller_pk, 
            Some(&fake_controller_authority)
        )?;

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }

    #[test]
    fn test_wrong_controller_authority_update_oracle_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (
            mut svm, 
            payer_and_authority, 
            controller_pk, 
            _permission_pda
        ) = setup_test_env()?;

        let (init_ixn, oracle_pda, price_feed) = get_init_oracle_ix(
            &mut svm,
            &payer_and_authority, 
            &controller_pk, 
            None
        )?;

        svm.send_transaction(Transaction::new_signed_with_payer(
            &[init_ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority],
            svm.latest_blockhash(),
        )).unwrap();

        let fake_controller_authority = Pubkey::new_unique();
        let new_authority = Keypair::new();
        let update_ixn = UpdateOracleBuilder::new()
            .controller(controller_pk)
            .controller_authority(fake_controller_authority)
            .authority(payer_and_authority.pubkey())
            .oracle(oracle_pda)
            .price_feed(price_feed)
            .feed_args(FeedArgs { oracle_type: 1})
            .new_authority(Some(new_authority.pubkey()))
            .instruction();

        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[update_ixn],
            Some(&payer_and_authority.pubkey()),
            &[&payer_and_authority, &new_authority],
            svm.latest_blockhash(),
        ));

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidControllerAuthority);

        Ok(())
    }
}