mod helpers;
mod subs;

use crate::subs::oracle::*;
use helpers::lite_svm_with_programs;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::accounts::Oracle;

#[cfg(test)]
mod tests {
    use solana_sdk::{
        account::Account, instruction::InstructionError, transaction::Transaction,
        transaction::TransactionError,
    };
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::{
        create_initialize_oracle_instruction, create_update_oracle_instruction,
        generated::{
            types::{
                ControllerStatus, FeedArgs, OracleUpdateEvent, PermissionStatus, SvmAlmControllerEvent,
            },
        },
    };
    use switchboard_on_demand::{
        Discriminator, PullFeedAccountData, ON_DEMAND_MAINNET_PID, PRECISION,
    };

    use crate::{
        helpers::{assert::assert_custom_error, setup_test_controller, TestContext},
        subs::{airdrop_lamports, derive_controller_authority_pda, initialize_contoller, manage_controller, manage_permission},
    };
    use borsh::BorshDeserialize;

    use super::*;
    use test_case::test_case;

    #[test]
    fn test_oracle_init_refresh_and_update_success() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();
        let authority2 = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &authority2.pubkey(), 1_000_000_000)?;

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            321u16, // Id
        )?;

        let nonce = Pubkey::new_unique();
        let new_feed = Pubkey::new_unique();
        let oracle_pda = derive_oracle_pda(&nonce);
        let oracle_type = 0;
        let mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();

        // Stub price feed data
        let update_slot = 1000_000;
        let update_price = 1_000_000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(&mut svm, &new_feed, update_price)?;

        // Initialize Oracle account
        let (tx_result, tx) = initialize_oracle(
            &mut svm,
            &controller_pk,
            &authority,
            &nonce,
            &new_feed,
            0,
            &mint,
            &quote_mint,
        );

        let meta = tx_result.map_err(|e| e.err.to_string())?;

        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let new_oracle = oracle.unwrap();
        assert_eq!(new_oracle.version, 1);
        assert_eq!(new_oracle.value, 0);
        assert_eq!(new_oracle.precision, PRECISION);
        assert_eq!(new_oracle.last_update_slot, 0);
        assert_eq!(new_oracle.controller, controller_pk);
        assert_eq!(new_oracle.base_mint, mint);
        assert_eq!(new_oracle.quote_mint, quote_mint);
        assert_eq!(new_oracle.reserved, [0; 64]);
        assert_eq!(new_oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(new_oracle.feeds[0].price_feed, new_feed);

        // assert event was emitted
        let expected_event = SvmAlmControllerEvent::OracleUpdate(OracleUpdateEvent {
            controller: controller_pk,
            oracle: oracle_pda,
            authority: authority.pubkey(),
            old_state: None,
            new_state: Some(new_oracle),
        });
        assert_contains_controller_cpi_event!(
            meta,
            tx.message.account_keys.as_slice(),
            expected_event
        );

        // Refresh Oracle account with price.
        refresh_oracle(&mut svm, &authority, &oracle_pda, &new_feed)?;

        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let refreshed_oracle = oracle.unwrap();
        assert_eq!(refreshed_oracle.version, 1);
        assert_eq!(refreshed_oracle.authority, authority.pubkey());
        assert_eq!(refreshed_oracle.nonce, nonce);
        assert_eq!(refreshed_oracle.value, update_price);
        assert_eq!(refreshed_oracle.precision, PRECISION);
        assert_eq!(refreshed_oracle.last_update_slot, update_slot);
        assert_eq!(refreshed_oracle.reserved, [0; 64]);
        assert_eq!(refreshed_oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(refreshed_oracle.feeds[0].price_feed, new_feed);

        // Update Oracle account with new authority.
        let (tx_result, tx) = update_oracle(
            &mut svm,
            &controller_pk,
            &authority,
            &oracle_pda,
            &new_feed,
            None, // keep oracle_type unchanged.
            Some(&authority2),
        );

        let meta = tx_result.map_err(|e| e.err.to_string())?;

        // Verify that only authority is updated.
        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let updated_authority_oracle = oracle.unwrap();
        assert_eq!(updated_authority_oracle.version, 1);
        assert_eq!(updated_authority_oracle.authority, authority2.pubkey());
        assert_eq!(updated_authority_oracle.nonce, nonce);
        assert_eq!(updated_authority_oracle.value, update_price);
        assert_eq!(updated_authority_oracle.precision, PRECISION);
        assert_eq!(updated_authority_oracle.last_update_slot, update_slot);
        assert_eq!(updated_authority_oracle.reserved, [0; 64]);
        assert_eq!(updated_authority_oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(updated_authority_oracle.feeds[0].price_feed, new_feed);

        // assert event was emitted for authority update
        let expected_event = SvmAlmControllerEvent::OracleUpdate(OracleUpdateEvent {
            controller: controller_pk,
            oracle: oracle_pda,
            authority: authority.pubkey(),
            old_state: Some(refreshed_oracle),
            new_state: Some(updated_authority_oracle.clone()),
        });
        assert_contains_controller_cpi_event!(
            meta,
            tx.message.account_keys.as_slice(),
            expected_event
        );

        // Update Oracle account with new feed.
        let new_feed2 = Pubkey::new_unique();
        let update_price = 2_500_000_000_000_000_000; // 2.5 (in 18 precision)
        set_price_feed(&mut svm, &new_feed2, update_price)?;
        let (tx_result, tx) = update_oracle(
            &mut svm,
            &controller_pk,
            &authority2,
            &oracle_pda,
            &new_feed2,
            Some(FeedArgs { oracle_type }),
            None,
        );
        let meta = tx_result.map_err(|e| e.err.to_string())?;

        // Verify that feed is updated.
        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let updated_feed_oracle = oracle.unwrap();
        assert_eq!(updated_feed_oracle.version, 1);
        assert_eq!(updated_feed_oracle.authority, authority2.pubkey());
        assert_eq!(updated_feed_oracle.nonce, nonce);
        assert_eq!(updated_feed_oracle.value, 0);
        assert_eq!(updated_feed_oracle.precision, PRECISION);
        assert_eq!(updated_feed_oracle.last_update_slot, 0);
        assert_eq!(updated_feed_oracle.reserved, [0; 64]);
        assert_eq!(updated_feed_oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(updated_feed_oracle.feeds[0].price_feed, new_feed2);

        // assert event was emitted for feed update
        let expected_event = SvmAlmControllerEvent::OracleUpdate(OracleUpdateEvent {
            controller: controller_pk,
            oracle: oracle_pda,
            authority: authority2.pubkey(),
            old_state: Some(updated_authority_oracle),
            new_state: Some(updated_feed_oracle),
        });
        assert_contains_controller_cpi_event!(
            meta,
            tx.message.account_keys.as_slice(),
            expected_event
        );

        Ok(())
    }

    #[test_log::test]
    fn test_initialize_oracle_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();

        // Airdrop to users
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        // Set up a controller
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            322u16, // Id
        )?;

        // Give authority freeze permissions
        manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &authority.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            true,  // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations,
            false, // can_suspend_permissions,
            false, // can_liquidate
        )?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Try to initialize oracle when frozen - should fail
        let nonce = Pubkey::new_unique();
        let new_feed = Pubkey::new_unique();
        let oracle_type = 0;
        let mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();

        let instruction = create_initialize_oracle_instruction(
            &controller_pk,
            &authority.pubkey(),
            &nonce,
            &new_feed,
            oracle_type,
            &mint,
            &quote_mint,
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test_log::test]
    fn test_update_oracle_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();
        let authority2 = Keypair::new();

        // Airdrop to users
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;
        airdrop_lamports(&mut svm, &authority2.pubkey(), 1_000_000_000)?;

        // Set up a controller
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            323u16, // Id
        )?;

        // Give authority freeze permissions (before initial oracle setup)
        manage_permission(
            &mut svm,
            &controller_pk,
            &authority,          // payer
            &authority,          // calling authority
            &authority.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            true,  // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            true,  // can_freeze,
            false, // can_unfreeze,
            false, // can_manage_reserves_and_integrations,
            false, // can_suspend_permissions,
            false, // can_liquidate
        )?;

        let nonce = Pubkey::new_unique();
        let new_feed = Pubkey::new_unique();
        let oracle_pda = derive_oracle_pda(&nonce);
        let oracle_type = 0;
        let mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();

        // Stub price feed data
        let update_slot = 1000_000;
        let update_price = 1_000_000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(&mut svm, &new_feed, update_price)?;

        // Initialize Oracle account
        initialize_oracle(
            &mut svm,
            &controller_pk,
            &authority,
            &nonce,
            &new_feed,
            oracle_type,
            &mint,
            &quote_mint,
        );

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &authority, // payer
            &authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        // Try to update oracle when frozen - should fail
        let ixn = create_update_oracle_instruction(
            &controller_pk,
            &authority.pubkey(),
            &oracle_pda,
            &new_feed,
            None,
            Some(&authority2.pubkey()),
        );

        let txn = Transaction::new_signed_with_payer(
            &[ixn],
            Some(&authority.pubkey()),
            &[&authority, &authority2],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test]
    fn test_oracle_init_fails_with_unsupported_oracle_type(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let nonce = Pubkey::new_unique();
        let new_feed = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();

        // Stub invalid price feed data: Incorrect length
        let mut serialized = Vec::with_capacity(8 + std::mem::size_of::<PullFeedAccountData>() - 1);
        serialized.extend_from_slice(&PullFeedAccountData::DISCRIMINATOR);

        svm.set_account(
            new_feed,
            Account {
                lamports: 1_000_000_000,
                data: serialized,
                owner: ON_DEMAND_MAINNET_PID,
                executable: false,
                rent_epoch: u64::MAX,
            },
        )?;

        // Initialize Oracle account
        let (tx_result, _) = initialize_oracle(
            &mut svm,
            &controller_pk,
            &super_authority,
            &nonce,
            &new_feed,
            0,
            &mint,
            &quote_mint,
        );

        assert_eq!(
            tx_result.err().expect("error").err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );

        let nonce = Pubkey::new_unique();
        let new_feed = Pubkey::new_unique();
        // Stub invalid price feed data: Incorrect discriminator
        let mut serialized = Vec::with_capacity(8 + std::mem::size_of::<PullFeedAccountData>());
        serialized.extend_from_slice(&[1u8; 8]);

        svm.set_account(
            new_feed,
            Account {
                lamports: 1_000_000_000,
                data: serialized,
                owner: ON_DEMAND_MAINNET_PID,
                executable: false,
                rent_epoch: u64::MAX,
            },
        )?;

        // Initialize Oracle account
        let (tx_result, _) = initialize_oracle(
            &mut svm,
            &controller_pk,
            &super_authority,
            &nonce,
            &new_feed,
            0,
            &mint,
            &quote_mint,
        );

        assert_eq!(
            tx_result.err().expect("error").err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );

        let nonce = Pubkey::new_unique();
        let new_feed = Pubkey::new_unique();
        // Stub invalid price feed data: Incorrect owner
        let mut serialized = Vec::with_capacity(8 + std::mem::size_of::<PullFeedAccountData>());
        serialized.extend_from_slice(&PullFeedAccountData::DISCRIMINATOR);

        svm.set_account(
            new_feed,
            Account {
                lamports: 1_000_000_000,
                data: serialized,
                owner: svm_alm_controller_client::SVM_ALM_CONTROLLER_ID,
                executable: false,
                rent_epoch: u64::MAX,
            },
        )?;

        // Initialize Oracle account
        let (tx_result, _) = initialize_oracle(
            &mut svm,
            &controller_pk,
            &super_authority,
            &nonce,
            &new_feed,
            0,
            &mint,
            &quote_mint,
        );

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidAccountData);

        // Initialize Oracle account with unsupported oracle_type
        let (tx_result, _) = initialize_oracle(
            &mut svm,
            &controller_pk,
            &super_authority,
            &nonce,
            &new_feed,
            1,
            &mint,
            &quote_mint,
        );

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::UnsupportedOracleType);

        Ok(())
    }
}
