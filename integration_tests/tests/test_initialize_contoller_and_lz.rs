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
use borsh::BorshDeserialize;
use bytemuck::checked::try_from_bytes;
use endpoint_client::types::MessagingReceipt;
use helpers::lite_svm_with_programs;
use solana_program::pubkey;
use solana_sdk::{signature::Keypair, signer::Signer};
use svm_alm_controller_client::generated::types::{
    ControllerStatus, IntegrationConfig, IntegrationStatus, PermissionStatus,
};
use svm_alm_controller_client::generated::types::{InitializeArgs, PushArgs, ReserveStatus};

#[cfg(test)]
mod tests {

    use std::ptr::null;

    use litesvm::LiteSVM;
    use oft_client::{
        instructions::SendInstructionArgs,
        oft302::{
            Oft302, Oft302Accounts, Oft302Programs, Oft302QuoteParams, Oft302SendAccounts,
            Oft302SendPrograms,
        },
    };
    use solana_client::rpc_client::RpcClient;
    use solana_sdk::{
        compute_budget::ComputeBudgetInstruction,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program, sysvar,
        transaction::Transaction,
    };
    use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
    use svm_alm_controller::{
        enums::IntegrationState, error::SvmAlmControllerErrors, state::controller,
    };
    use svm_alm_controller_client::generated::{
        instructions::{PushBuilder, ResetLzPushInFlightBuilder},
        types::LzBridgeConfig,
    };

    use crate::{
        helpers::{
            assert::assert_custom_error,
            cctp::evm_address_to_solana_pubkey,
            constants::{
                DEVNET_RPC, LZ_DESTINATION_DOMAIN_EID, LZ_ENDPOINT_PROGRAM_ID, LZ_USDS_ESCROW,
                LZ_USDS_OFT_PROGRAM_ID, LZ_USDS_OFT_STORE_PUBKEY, LZ_USDS_PEER_CONFIG_PUBKEY,
            },
            spl::setup_token_account,
            utils::get_program_return_data,
        },
        subs::{
            derive_controller_authority_pda, derive_permission_pda, derive_reserve_pda,
            fetch_integration_account, fetch_reserve_account, ReserveKeys,
        },
    };

    use super::*;

    fn setup_env(
        svm: &mut LiteSVM,
    ) -> Result<(Pubkey, Pubkey, Keypair, ReserveKeys), Box<dyn std::error::Error>> {
        let authority: Keypair = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(svm, &authority.pubkey(), 1_000_000_000)?;

        // Create an ATA for the USDC account
        let _authority_usds_ata = initialize_ata(
            svm,
            &authority,
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
        )?;

        // Cheat to give the authority some USDC
        edit_ata_amount(
            svm,
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            1_000_000_000,
        )?;

        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            svm,
            &authority,
            &authority,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let controller_authority = derive_controller_authority_pda(&controller_pk);

        // Update the authority to have all permissions
        let _ = manage_permission(
            svm,
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

        // Initialize a reserve for the token
        let usds_reserve_keys = initialize_reserve(
            svm,
            &controller_pk,
            &USDS_TOKEN_MINT_PUBKEY, // mint
            &authority,              // payer
            &authority,              // authority
            ReserveStatus::Active,
            1_000_000_000_000, // rate_limit_slope
            1_000_000_000_000, // rate_limit_max_outflow
            &spl_token::ID,
        )?;

        // Transfer funds into the reserve
        transfer_tokens(
            svm,
            &authority,
            &authority,
            &USDS_TOKEN_MINT_PUBKEY,
            &controller_authority,
            500_000_000,
        )?;

        // Serialize the destination address appropriately
        let evm_address = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        // Initialize an integration
        let lz_usds_eth_bridge_integration_pk = initialize_integration(
            svm,
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
                token_escrow: LZ_USDS_ESCROW,
                oft_store: LZ_USDS_OFT_STORE_PUBKEY,
                peer_config: LZ_USDS_PEER_CONFIG_PUBKEY,
                padding: [0; 28],
            }),
            &InitializeArgs::LzBridge {
                desination_address: destination_address,
                destination_eid: LZ_DESTINATION_DOMAIN_EID,
            },
        )?;
        Ok((
            controller_pk,
            lz_usds_eth_bridge_integration_pk,
            authority,
            usds_reserve_keys,
        ))
    }

    fn create_lz_push_ix(
        controller: &Pubkey,
        integration: &Pubkey,
        authority: &Keypair,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        let calling_permission_pda = derive_permission_pda(&controller, &authority.pubkey());
        let reserve_pda = derive_reserve_pda(&controller, &USDS_TOKEN_MINT_PUBKEY);
        let controller_authority = derive_controller_authority_pda(controller);
        let vault = get_associated_token_address_with_program_id(
            &controller_authority,
            &USDS_TOKEN_MINT_PUBKEY,
            &pinocchio_token::ID.into(),
        );
        let authority_token_account = get_associated_token_address_with_program_id(
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            &pinocchio_token::ID.into(),
        );

        let amount = 2000;
        let main_ixn = PushBuilder::new()
            .push_args(PushArgs::LzBridge { amount })
            .controller(*controller)
            .controller_authority(controller_authority)
            .authority(authority.pubkey())
            .permission(calling_permission_pda)
            .integration(*integration)
            .reserve_a(reserve_pda)
            .reserve_b(reserve_pda)
            .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
            .add_remaining_accounts(&[
                AccountMeta {
                    pubkey: USDS_TOKEN_MINT_PUBKEY,
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: vault,
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: authority_token_account,
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: pinocchio_token::ID.into(),
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: pinocchio_associated_token_account::ID.into(),
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: system_program::ID,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: sysvar::instructions::ID,
                    is_signer: false,
                    is_writable: false,
                },
            ])
            .instruction();

        Ok(main_ixn)
    }

    async fn create_oft_send_ix(
        svm: &mut LiteSVM,
        controller: &Pubkey,
        integration: &Pubkey,
        authority: &Keypair,
        token_source: Pubkey,
        amount: u64,
        skip_setup: bool,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        // Serialize the destination address appropriately
        let evm_address = "0x0804a6e2798f42c7f3c97215ddf958d5500f8ec8";
        let destination_address = evm_address_to_solana_pubkey(evm_address);

        let oft302 = Oft302::new(LZ_USDS_OFT_PROGRAM_ID, DEVNET_RPC.to_owned());
        let (native_fee, lz_token_fee) = if !skip_setup {
            let quote_accounts = Oft302Accounts {
                // dummy payer for devnet fetch
                payer: pubkey!("Fty7h4FYAN7z8yjqaJExMHXbUoJYMcRjWYmggSxLbHp8"),
                token_mint: USDS_TOKEN_MINT_PUBKEY,
                token_escrow: LZ_USDS_ESCROW,
                peer_address: None,
            };
            let quote_params = Oft302QuoteParams {
                dst_eid: LZ_DESTINATION_DOMAIN_EID,
                to: destination_address.to_bytes(),
                amount_ld: amount,
                min_amount_ld: amount,
            };
            let quote = oft302
                .quote(
                    quote_accounts.clone(),
                    quote_params.clone(),
                    Oft302Programs { endpoint: None },
                    vec![],
                )
                .await
                .unwrap();
            (quote.native_fee, quote.lz_token_fee)
        } else {
            (0, 0)
        };

        let send_accs = Oft302SendAccounts {
            payer: authority.pubkey(),
            token_mint: USDS_TOKEN_MINT_PUBKEY,
            token_escrow: LZ_USDS_ESCROW,
            token_source,
            peer_address: None,
        };
        let send_params = SendInstructionArgs {
            dst_eid: LZ_DESTINATION_DOMAIN_EID,
            to: destination_address.to_bytes(),
            amount_ld: amount,
            min_amount_ld: amount,
            options: vec![],
            compose_msg: None,
            native_fee: native_fee,
            lz_token_fee: lz_token_fee,
        };
        let send_programs = Oft302SendPrograms {
            endpoint: Some(LZ_ENDPOINT_PROGRAM_ID),
            token: Some(pinocchio_token::ID.into()),
        };
        let send_ixn = oft302
            .send(send_accs, send_params, send_programs, vec![])
            .await?;

        // Load required Layer Zero accounts from devnet into litesvm environment.
        if !skip_setup {
            let rpc = RpcClient::new(DEVNET_RPC);
            for acc in send_ixn.accounts.clone() {
                match rpc.get_account(&acc.pubkey) {
                    Ok(account) => {
                        if !account.executable {
                            svm.set_account(acc.pubkey, account)?
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch account {}: {:?}", acc.pubkey, e);
                    }
                }
            }
        }
        Ok(send_ixn)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lz_push_with_oft_send_success() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller_pk, lz_usds_eth_bridge_integration_pk, authority, reserve_keys) =
            setup_env(&mut svm)?;

        // Push the integration -- i.e. bridge using LZ OFT
        let amount = 2000;
        let result = push_integration(
            &mut svm,
            &controller_pk,
            &lz_usds_eth_bridge_integration_pk,
            &authority,
            &PushArgs::LzBridge { amount },
            false,
        )
        .await?;

        // Check that OFT return data exists and amount matches.
        let return_data =
            get_program_return_data(result.clone().unwrap().logs, &LZ_USDS_OFT_PROGRAM_ID).unwrap();
        let (messaging_receipt, oft_receipt) =
            <(MessagingReceipt, oft_client::types::OFTReceipt)>::try_from_slice(&return_data)
                .map_err(|err| format!("Failed to parse result: {}", err))
                .unwrap();
        assert_eq!(oft_receipt.amount_sent_ld, amount);
        assert_eq!(oft_receipt.amount_received_ld, amount);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lz_push_tx_introspection_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller, integration, authority, _reserve_keys) = setup_env(&mut svm)?;

        let authority_token_account = get_associated_token_address_with_program_id(
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            &pinocchio_token::ID.into(),
        );
        let amount = 2000;

        let main_ixn = create_lz_push_ix(&controller, &integration, &authority)?;
        let send_ixn = create_oft_send_ix(
            &mut svm,
            &controller,
            &integration,
            &authority,
            authority_token_account,
            amount,
            true,
        )
        .await?;
        let cu_limit_ixn: Instruction = ComputeBudgetInstruction::set_compute_unit_limit(400_000);

        // Expect failure without send_ixn.
        let txn = Transaction::new_signed_with_payer(
            &[main_ixn.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::UnauthorizedAction);

        // Expect failure without send_ixn as last.
        let txn = Transaction::new_signed_with_payer(
            &[main_ixn.clone(), send_ixn, cu_limit_ixn.clone()],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when token_escrow doesn't match.
        let random_ata = get_associated_token_address_with_program_id(
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            &pinocchio_token::ID.into(),
        );
        let send_ixn = create_oft_send_ix(
            &mut svm,
            &controller,
            &integration,
            &authority,
            Pubkey::new_unique(),
            amount,
            true,
        )
        .await?;
        let txn = Transaction::new_signed_with_payer(
            &[main_ixn.clone(), send_ixn],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        // Expect failure when send amount doesn't match.
        let send_ixn = create_oft_send_ix(
            &mut svm,
            &controller,
            &integration,
            &authority,
            authority_token_account,
            111,
            true,
        )
        .await?;
        let txn = Transaction::new_signed_with_payer(
            &[main_ixn.clone(), send_ixn],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::InvalidInstructions);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiple_push_with_single_send_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let (controller, integration, authority, _reserve_keys) = setup_env(&mut svm)?;

        let authority_token_account = get_associated_token_address_with_program_id(
            &authority.pubkey(),
            &USDS_TOKEN_MINT_PUBKEY,
            &pinocchio_token::ID.into(),
        );
        let amount = 2000;

        let lz_push_ixn = create_lz_push_ix(&controller, &integration, &authority)?;
        let send_ixn = create_oft_send_ix(
            &mut svm,
            &controller,
            &integration,
            &authority,
            authority_token_account,
            amount,
            false,
        )
        .await?;
        let reset_ix = ResetLzPushInFlightBuilder::new()
            .controller(controller)
            .integration(integration)
            .instruction();

        let txn = Transaction::new_signed_with_payer(
            &[lz_push_ixn.clone(), lz_push_ixn.clone(), send_ixn, reset_ix],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert_custom_error(&tx_result, 1, SvmAlmControllerErrors::LZPushInFlight);
        Ok(())
    }
}
