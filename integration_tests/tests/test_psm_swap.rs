mod helpers;
mod subs;

#[cfg(test)]
mod tests {
    use crate::{
        assert_contains_controller_cpi_event,
        helpers::{setup_test_controller, TestContext},
        subs::{
            derive_controller_authority_pda, edit_token_amount, fetch_integration_account,
            initialize_mint, setup_pool_with_token,
        },
        test_invalid_accounts,
    };
    use borsh::BorshDeserialize;
    use psm_client::accounts::{PsmPool, Token};
    use solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        pubkey::Pubkey,
        signer::Signer,
        transaction::{Transaction, TransactionError},
    };
    use spl_token::ID as TOKEN_PROGRAM_ID;
    use svm_alm_controller_client::{
        generated::types::{
            IntegrationConfig, IntegrationState, IntegrationStatus, IntegrationUpdateEvent,
            PsmSwapConfig, SvmAlmControllerEvent,
        },
        initialize_integration::create_psm_swap_initialize_integration_instruction,
    };
    use test_case::test_case;

    use borsh::BorshSerialize;

    fn serialize_to_vec<T: BorshSerialize>(data: &T) -> Result<Vec<u8>, std::io::Error> {
        let mut buf = Vec::new();
        data.serialize(&mut buf)?;
        Ok(buf)
    }

    #[test]
    fn test_psm_swap_init_success() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );
        let rate_limit_slope = 10_000_000_000;
        let rate_limit_max_outflow = 10_000_000_000;
        let integration_status = IntegrationStatus::Active;
        let permit_liquidation = true;
        let description = "psm swap";

        let (init_psm_integration_ix, integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                description,
                integration_status,
                rate_limit_slope,
                rate_limit_max_outflow,
                permit_liquidation,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone()).map_err(|e| {
            println!("logs: {}", e.meta.pretty_logs());
            e.err.to_string()
        })?;

        let integration = fetch_integration_account(&svm, &integration_pda)
            .expect("integration should exist")
            .unwrap();

        let clock = svm.get_sysvar::<Clock>();

        assert_eq!(integration.controller, controller_pk);
        assert_eq!(integration.status, IntegrationStatus::Active);
        assert_eq!(integration.rate_limit_slope, rate_limit_slope);
        assert_eq!(integration.rate_limit_max_outflow, rate_limit_max_outflow);
        assert_eq!(
            integration.rate_limit_outflow_amount_available,
            rate_limit_max_outflow
        );
        assert_eq!(integration.rate_limit_remainder, 0);
        assert_eq!(integration.permit_liquidation, permit_liquidation);
        assert_eq!(integration.last_refresh_timestamp, clock.unix_timestamp);
        assert_eq!(integration.last_refresh_slot, clock.slot);

        match integration.clone().config {
            IntegrationConfig::PsmSwap(config) => {
                assert_eq!(
                    config,
                    PsmSwapConfig {
                        psm_token: token_pda,
                        psm_pool: pool_pda,
                        mint: liquidity_mint,
                        padding: [0; 128]
                    }
                );
            }
            _ => panic!("invalid config"),
        }

        let psm_state = match integration.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        assert_eq!(psm_state.liquidity_supplied, 0);

        let expected_event = SvmAlmControllerEvent::IntegrationUpdate(IntegrationUpdateEvent {
            controller: controller_pk,
            integration: integration_pda,
            authority: super_authority.pubkey(),
            old_state: None,
            new_state: Some(integration),
        });
        assert_contains_controller_cpi_event!(
            tx_result,
            transaction.message.account_keys.as_slice(),
            expected_event
        );

        Ok(())
    }

    enum InvalidPsmStates {
        LiquidityOwnerMismatch,
        PoolMismatch,
        MintMismatch,
        VaultMismatch,
    }

    #[test_case(InvalidPsmStates::LiquidityOwnerMismatch; "liquidity owner mismatch")]
    #[test_case(InvalidPsmStates::PoolMismatch; "pool mismatch")]
    #[test_case(InvalidPsmStates::MintMismatch; "mint mismatch")]
    #[test_case(InvalidPsmStates::VaultMismatch; "vault mismatch")]
    fn test_psm_swap_init_invalid_psm_state_fails(
        invalid_state: InvalidPsmStates,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );

        let mut psm_pool_acc = svm.get_account(&pool_pda).unwrap();
        let mut psm_pool = PsmPool::from_bytes(&psm_pool_acc.data[1..])?;
        let mut psm_token_pda_acc = svm.get_account(&token_pda).unwrap();
        let mut psm_token = Token::from_bytes(&psm_token_pda_acc.data[1..])?;
        match invalid_state {
            InvalidPsmStates::LiquidityOwnerMismatch => {
                psm_pool.liquidity_owner = Pubkey::default()
            }
            InvalidPsmStates::MintMismatch => psm_token.mint = Pubkey::default(),
            InvalidPsmStates::PoolMismatch => psm_token.pool = Pubkey::default(),
            InvalidPsmStates::VaultMismatch => psm_token.pool = Pubkey::default(),
        }
        psm_pool_acc.data[1..].copy_from_slice(&serialize_to_vec(&psm_pool)?);
        svm.set_account(pool_pda, psm_pool_acc).unwrap();
        psm_token_pda_acc.data[1..].copy_from_slice(&serialize_to_vec(&psm_token)?);
        svm.set_account(token_pda, psm_token_pda_acc).unwrap();

        let (init_psm_integration_ix, _integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(transaction.clone());

        assert_eq!(
            tx_result.err().unwrap().err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );

        Ok(())
    }

    #[test]
    fn test_psm_swap_init_invalid_account_owners_fails() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );

        let (init_psm_integration_ix, _integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        test_invalid_accounts!(
            svm.clone(),
            super_authority.pubkey(),
            vec![Box::new(&super_authority)],
            init_psm_integration_ix.clone(),
            {
                8 => invalid_owner(InstructionError::InvalidAccountOwner, "PsmPool invalid owner"),
                9 => invalid_owner(InstructionError::InvalidAccountOwner, "PsmToken invalid owner"),
                10 => invalid_owner(InstructionError::InvalidAccountOwner, "PsmTokenVault invalid owner"),
                11 => invalid_owner(InstructionError::InvalidAccountOwner, "Mint invalid owner"),
            }
        );

        Ok(())
    }

    #[test]
    fn test_psm_swap_init_psm_vault_with_balance() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        let controller_authority = derive_controller_authority_pda(&controller_pk);

        let liquidity_mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &TOKEN_PROGRAM_ID,
            None,
            None,
        )?;

        // initialize a PSM Pool
        let (pool_pda, token_pda, token_vault) = setup_pool_with_token(
            &mut svm,
            &super_authority,
            &liquidity_mint,
            false,
            false,
            &controller_authority,
        );

        let vault_balance = 1_000_000;
        edit_token_amount(&mut svm, &token_vault, vault_balance)?;

        let (init_psm_integration_ix, integration_pda) =
            create_psm_swap_initialize_integration_instruction(
                &super_authority.pubkey(),
                &controller_pk,
                &super_authority.pubkey(),
                &liquidity_mint,
                "psm swap",
                IntegrationStatus::Active,
                10_000_000_000,
                10_000_000_000,
                true,
                &pool_pda,
                &token_pda,
                &token_vault,
            );

        let transaction = Transaction::new_signed_with_payer(
            &[init_psm_integration_ix],
            Some(&super_authority.pubkey()),
            &[super_authority.insecure_clone()],
            svm.latest_blockhash(),
        );
        svm.send_transaction(transaction).unwrap();

        let integration = fetch_integration_account(&svm, &integration_pda)
            .expect("integration should exist")
            .unwrap();

        let psm_state = match integration.clone().state {
            IntegrationState::PsmSwap(state) => state,
            _ => panic!("invalid state"),
        };
        assert_eq!(psm_state.liquidity_supplied, vault_balance);

        Ok(())
    }
}
