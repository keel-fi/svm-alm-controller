use helpers::lite_svm_with_programs;
use svm_alm_controller_client::instructions::{InitializeControllerBuilder, ManagePermissionBuilder, InializeIntegrationBuilder};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};
use solana_sdk::pubkey::Pubkey;
use svm_alm_controller_client::programs::SVM_ALM_CONTROLLER_ID;
use solana_sdk::system_program;
use solana_keccak_hasher::hash;
use borsh::BorshDeserialize;
use svm_alm_controller_client::{accounts::{Controller, Permission, Integration}, types::{ControllerStatus,PermissionStatus, IntegrationConfig, IntegrationStatus}};
use pinocchio_token::state::token; 
mod helpers;
use helpers::print_inner_instructions;

#[cfg(test)]
mod tests {



    use solana_sdk::{feature_set::spl_token_v2_multisig_fix, instruction::AccountMeta, program_pack::Pack, rent::Rent};
    use svm_alm_controller_client::{instructions::{PushBuilder, SyncBuilder}, types::{IntegrationState, IntegrationType, SplTokenExternalConfig, SplTokenVaultConfig, SplTokenVaultState}};

    use crate::helpers::print_inner_instructions;

    use super::*;

    #[test_log::test]
    fn initialize_controller_success() {
        let authority = Keypair::new();
        let mut svm = lite_svm_with_programs();
        
        // Airdrop to payer
        svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

        let status = ControllerStatus::Active; //  Active
        let id = 1u16;
        let (controller_pda, _controller_bump) = Pubkey::find_program_address(
            &[
                b"controller",
                &id.to_le_bytes(),
            ],
            &Pubkey::from(SVM_ALM_CONTROLLER_ID),
        );
        let (permission_pda, _permission_bump) = Pubkey::find_program_address(
            &[
                b"permission",
                &controller_pda.to_bytes(),
                &authority.pubkey().to_bytes(),
            ],
            &Pubkey::from(SVM_ALM_CONTROLLER_ID),
        );
    
        let ix = InitializeControllerBuilder::new()
            .id(id) 
            .status(status) 
            .payer(authority.pubkey())
            .authority(authority.pubkey())
            .controller(controller_pda)
            .permission(permission_pda)
            .system_program(system_program::ID)
            .instruction();

        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&authority.pubkey()),
            &[&authority], 
            svm.latest_blockhash(),
        );

        let tx_res = svm.send_transaction(transaction).unwrap();
        // println!("{:#?}", tx_res);
        // print_inner_instructions(&tx_res);


        let controller_info = svm.get_account(&controller_pda).unwrap();
        let permission_info = svm.get_account(&permission_pda).unwrap();

        let controller = Controller::try_from_slice(&controller_info.data[1..]).unwrap(); // TODO: Fix Discriminator
        let permission = Permission::try_from_slice(&permission_info.data[1..]).unwrap(); // TODO: Fix Discriminator

        assert_eq!(controller.status, ControllerStatus::Active);
        assert_eq!(controller.id, id);
        assert_eq!(permission.authority, authority.pubkey());
        assert_eq!(permission.controller, controller_pda);



        // MANAGE_PERMISSION - create a new authority

  
        let new_authority = Keypair::new();

       
        let (new_permission_pda, _new_permission_bump) = Pubkey::find_program_address(
            &[
                b"permission",
                &controller_pda.to_bytes(),
                &new_authority.pubkey().to_bytes(),
            ],
            &Pubkey::from(SVM_ALM_CONTROLLER_ID),
        );
    
        let ix = ManagePermissionBuilder::new()
            .status(PermissionStatus::Active) 
            .can_execute_swap(true)
            .can_manage_permissions(true)
            .can_invoke_external_transfer(true)
            .can_reallocate(true)
            .can_freeze(true)
            .can_unfreeze(true)
            .can_manage_integrations(true)
            .payer(authority.pubkey())
            .controller(controller_pda)
            .super_authority(authority.pubkey())
            .super_permission(permission_pda)
            .authority(new_authority.pubkey())
            .permission(new_permission_pda)
            .system_program(system_program::ID)
            .instruction();

        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&authority.pubkey()),
            &[&authority],  // Both payer and authority need to sign
            svm.latest_blockhash(),
        );

        let tx_res = svm.send_transaction(transaction).unwrap();
        

        let controller_info = svm.get_account(&controller_pda).unwrap();
        let new_permission_info = svm.get_account(&new_permission_pda).unwrap();
        let permission_info = svm.get_account(&permission_pda).unwrap();


        let controller = Controller::try_from_slice(&controller_info.data[1..]).unwrap(); // TODO: Fix Discriminator
        let permission = Permission::try_from_slice(&permission_info.data[1..]).unwrap(); // TODO: Fix Discriminator
        let new_permission = Permission::try_from_slice(&new_permission_info.data[1..]).unwrap(); // TODO: Fix Discriminator


        // println!("{:?}", controller);
        // println!("{:?}", new_permission);
        // println!("{:?}", permission);

        assert_eq!(new_permission.authority, new_authority.pubkey());
        assert_eq!(new_permission.controller, controller_pda);
        assert_eq!(new_permission.status, PermissionStatus::Active);
        assert_eq!(new_permission.can_execute_swap, true);
        assert_eq!(new_permission.can_freeze, true);
        assert_eq!(new_permission.can_manage_permissions, true);
        assert_eq!(new_permission.can_manage_integrations, true);
        assert_eq!(new_permission.can_reallocate, true);
        assert_eq!(new_permission.can_invoke_external_transfer, true);
        assert_eq!(new_permission.can_unfreeze, true);


        // MANAGE_PERMISSION - update existing (own) permissions


    
        let ix = ManagePermissionBuilder::new()
            .status(PermissionStatus::Active) 
            .can_execute_swap(true)
            .can_manage_permissions(true)
            .can_manage_integrations(true)
            .can_invoke_external_transfer(true)
            .can_reallocate(true)
            .can_freeze(true)
            .can_unfreeze(true)
            .payer(authority.pubkey())
            .controller(controller_pda)
            .super_authority(authority.pubkey())
            .super_permission(permission_pda)
            .authority(authority.pubkey())
            .permission(permission_pda)
            .system_program(system_program::ID)
            .instruction();

        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&authority.pubkey()),
            &[&authority],  // Both payer and authority need to sign
            svm.latest_blockhash(),
        );

        let tx_res = svm.send_transaction(transaction).unwrap();

        let controller_info = svm.get_account(&controller_pda).unwrap();
        let new_permission_info = svm.get_account(&new_permission_pda).unwrap();
        let permission_info = svm.get_account(&permission_pda).unwrap();


        let controller = Controller::try_from_slice(&controller_info.data[1..]).unwrap(); // TODO: Fix Discriminator
        let permission = Permission::try_from_slice(&permission_info.data[1..]).unwrap(); // TODO: Fix Discriminator

        assert_eq!(permission.authority, authority.pubkey());
        assert_eq!(permission.controller, controller_pda);
        assert_eq!(permission.status, PermissionStatus::Active);
        assert_eq!(permission.can_execute_swap, true);
        assert_eq!(permission.can_freeze, true);
        assert_eq!(permission.can_manage_permissions, true);
        assert_eq!(permission.can_manage_integrations, true);
        assert_eq!(permission.can_reallocate, true);
        assert_eq!(permission.can_invoke_external_transfer, true);
        assert_eq!(permission.can_unfreeze, true);


        // CREATE A MINT FOR TESTING 

        let mint_kp = Keypair::new();
        let mint_pk = mint_kp.pubkey();
        let mint_len = spl_token::state::Mint::LEN;

        let create_acc_ins = solana_system_interface::instruction::create_account(
            &authority.pubkey(),
            &mint_pk,
            svm.minimum_balance_for_rent_exemption(mint_len),
            mint_len as u64,
            &spl_token::id(),
        );

        let init_mint_ins = spl_token::instruction::initialize_mint2(
            &spl_token::id(), 
            &mint_pk, 
            &authority.pubkey(),
            None, 
            6
        ).unwrap();
        let tx_result = svm.send_transaction(
            Transaction::new_signed_with_payer(
                &[create_acc_ins, init_mint_ins],
                Some(&authority.pubkey()),
                &[&authority, &mint_kp],
                svm.latest_blockhash(),
            )
        );
        assert!(tx_result.is_ok());
       
        let mint_acc = svm.get_account(&mint_kp.pubkey());
        let mint = spl_token::state::Mint::unpack(&mint_acc.unwrap().data).unwrap();

        assert_eq!(mint.decimals, 6);
        assert_eq!(mint.mint_authority, Some(authority.pubkey()).into());
    

    

        
        // INITIALZE INTEGRATION 

        let vault = spl_associated_token_account_client::address::get_associated_token_address(
            &controller_pda,
            &mint_pk,
        );

        let spl_token_program_id = Pubkey::from(pinocchio_token::ID);
        let spl_token_config = SplTokenVaultConfig{
            vault: vault, 
            mint: mint_pk, 
            program: pinocchio_token::ID.into(), 
            padding: [0u8;96] 
        };
         
        let spl_token_config_bytes_to_hash: Vec<u8> = [
            &[1u8][..], // SplTokenVault
            &spl_token_config.program.to_bytes()[..],
            &spl_token_config.mint.to_bytes()[..],
            &spl_token_config.vault.to_bytes()[..],
            &spl_token_config.padding[..]
        ].concat();
        let spl_token_config_hash = hash(spl_token_config_bytes_to_hash.as_slice()).to_bytes();

        let (vault_integration_pda, _integration_bump) = Pubkey::find_program_address(
            &[
                b"integration",
                &controller_pda.to_bytes(),
                &spl_token_config_hash
            ],
            &Pubkey::from(SVM_ALM_CONTROLLER_ID),
        );

        // Create a fixed-size array with zeros
        let mut description = [0u8; 32];
        let source = "USDC VAULT".as_bytes();
        description[..source.len()].copy_from_slice(source);


        let remaining_accounts = [
            AccountMeta { pubkey: Pubkey::from(spl_token_config.mint), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(spl_token_config.vault), is_signer: false, is_writable: true },
            AccountMeta { pubkey: Pubkey::from(pinocchio_token::ID), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(pinocchio_associated_token_account::ID), is_signer: false, is_writable: false },
        ];

        // Create initialization integration instruction
        let ix = InializeIntegrationBuilder::new()
            .status(IntegrationStatus::Active) 
            .description(description)
            .integration_type(IntegrationType::SplTokenVault)
            .payer(authority.pubkey())
            .controller(controller_pda)
            .authority(authority.pubkey())
            .permission(permission_pda)
            .integration(vault_integration_pda)
            .lookup_table(system_program::ID)
            .system_program(system_program::ID)
            .add_remaining_accounts(&remaining_accounts)
            .instruction();

        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&authority.pubkey()),
            &[&authority], 
            svm.latest_blockhash(),
        );

        let tx_res = svm.send_transaction(transaction).unwrap();

        let integration_info = svm.get_account(&vault_integration_pda).unwrap();
        let integration = Integration::try_from_slice(&integration_info.data[1..]).unwrap(); // TODO: Fix Discriminator

        println!("{:?}", integration);

        assert_eq!(integration.controller, controller_pda);
        assert_eq!(integration.config, IntegrationConfig::SplTokenVault(spl_token_config.clone()));
        assert_eq!(integration.hash, spl_token_config_hash);

        let last_refresh_timestamp = match integration.state {
            IntegrationState::SplTokenVault(state) => state.last_refresh_timestamp,
            _ => {
                eprintln!("Unexpected integration state");
                return;
            }
        };


        // MINT TOKENS TO TEST SYNC

        let mint_amount = 100_000_000;

        let create_ata_ins = spl_associated_token_account_client::instruction::create_associated_token_account_idempotent(
            &authority.pubkey(),
            &controller_pda,
            &mint_pk, 
            &spl_token::id(), 
        );

        let mint_to_ins = spl_token::instruction::mint_to(
            &spl_token::id(), 
            &mint_pk, 
            &vault,
            &authority.pubkey(),
            &[&authority.pubkey()],
            mint_amount
        ).unwrap();

        let tx_result = svm.send_transaction(
            Transaction::new_signed_with_payer(
                &[create_ata_ins, mint_to_ins],
                Some(&authority.pubkey()),
                &[&authority],
                svm.latest_blockhash(),
            )
        );
        assert!(tx_result.is_ok());
       
        let vault_acc = svm.get_account(&vault);
        let mint_acc = svm.get_account(&mint_kp.pubkey());
        let mint_state = spl_token::state::Mint::unpack(&mint_acc.unwrap().data).unwrap();
        let vault_state = spl_token::state::Account::unpack(&vault_acc.unwrap().data).unwrap();

        assert_eq!(mint_state.supply, mint_amount);
        assert_eq!(vault_state.amount, mint_amount);


            
        
        /// TEST SYNC SPL VAULT
        

        let remaining_accounts = [
            AccountMeta { pubkey: Pubkey::from(spl_token_config.vault), is_signer: false, is_writable: false },
        ];

        // Create initialization integration instruction
        let sync_ix = SyncBuilder::new()
            .controller(controller_pda)
            .integration(vault_integration_pda)
            .add_remaining_accounts(&remaining_accounts)
            .instruction();

        let transaction = Transaction::new_signed_with_payer(
            &[sync_ix],
            Some(&authority.pubkey()),
            &[&authority], 
            svm.latest_blockhash(),
        );

        let tx_res = svm.send_transaction(transaction).unwrap();
        println!("{:#?}", tx_res.logs);

        let integration_info = svm.get_account(&vault_integration_pda).unwrap();
        let integration = Integration::try_from_slice(&integration_info.data[1..]).unwrap(); // TODO: Fix Discriminator
        println!("{:?}", integration);

        let (current_refresh_timestamp, current_balance) = match integration.state {
            IntegrationState::SplTokenVault(state) => (state.last_refresh_timestamp, state.last_balance),
            _ => {
                eprintln!("Unexpected integration state");
                return;
            }
        };

        // assert_ne!(current_refresh_timestamp, last_refresh_timestamp); // Slot and timestamp don't update in LiteSVM
        assert_eq!(current_balance, mint_amount);



          
        // INITIALZE EXTERNAL INTEGRATION 

        let recipient = Keypair::new();
        let receipient_token_account = spl_associated_token_account_client::address::get_associated_token_address(
            &recipient.pubkey(),
            &mint_pk,
        );

        let external_config = SplTokenExternalConfig{
            program: pinocchio_token::ID.into(), 
            mint: mint_pk, 
            recipient: recipient.pubkey(),
            token_account: receipient_token_account, 
            padding: [0u8;64] 
        };
         
        let external_config_bytes_to_hash: Vec<u8> = [
            &[2u8][..], // SplTokenExternal
            &external_config.program.to_bytes()[..],
            &external_config.mint.to_bytes()[..],
            &external_config.recipient.to_bytes()[..],
            &external_config.token_account.to_bytes()[..],
            &external_config.padding[..]
        ].concat();
        let external_config_hash = hash(external_config_bytes_to_hash.as_slice()).to_bytes();

        let (external_integration_pda, _external_integration_bump) = Pubkey::find_program_address(
            &[
                b"integration",
                &controller_pda.to_bytes(),
                &external_config_hash
            ],
            &Pubkey::from(SVM_ALM_CONTROLLER_ID),
        );

        // Create a fixed-size array with zeros
        let mut description = [0u8; 32];
        let source = "USDC COLD WALLET".as_bytes();
        description[..source.len()].copy_from_slice(source);


        let external_initialize_remaining_accounts = [
            AccountMeta { pubkey: Pubkey::from(external_config.mint), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(external_config.recipient), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(external_config.token_account), is_signer: false, is_writable: true },
            AccountMeta { pubkey: Pubkey::from(pinocchio_token::ID), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(pinocchio_associated_token_account::ID), is_signer: false, is_writable: false },
        ];

        // Create initialization integration instruction
        let ix = InializeIntegrationBuilder::new()
            .status(IntegrationStatus::Active) 
            .description(description)
            .integration_type(IntegrationType::SplTokenExternal)
            .payer(authority.pubkey())
            .controller(controller_pda)
            .authority(authority.pubkey())
            .permission(permission_pda)
            .integration(external_integration_pda)
            .lookup_table(system_program::ID)
            .system_program(system_program::ID)
            .add_remaining_accounts(&external_initialize_remaining_accounts)
            .instruction();

        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&authority.pubkey()),
            &[&authority], 
            svm.latest_blockhash(),
        );

        let tx_res = svm.send_transaction(transaction).unwrap();

        let external_integration_info = svm.get_account(&external_integration_pda).unwrap();
        let external_integration = Integration::try_from_slice(&external_integration_info.data[1..]).unwrap(); // TODO: Fix Discriminator

        println!("{:?}", external_integration);

        assert_eq!(external_integration.controller, controller_pda);
        assert_eq!(external_integration.config, IntegrationConfig::SplTokenExternal(external_config.clone()));
        assert_eq!(external_integration.hash, external_config_hash);



        // EXTERNAL TRANSFER TO THE COLD WALLET
        

        let external_push_remaining_accounts = [
            AccountMeta { pubkey: Pubkey::from(vault_integration_pda), is_signer: false, is_writable: true },
            AccountMeta { pubkey: Pubkey::from(external_config.mint), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(vault), is_signer: false, is_writable: true },
            AccountMeta { pubkey: Pubkey::from(external_config.recipient), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(external_config.token_account), is_signer: false, is_writable: true },
            AccountMeta { pubkey: Pubkey::from(pinocchio_token::ID), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(pinocchio_associated_token_account::ID), is_signer: false, is_writable: false },
            AccountMeta { pubkey: Pubkey::from(system_program::ID), is_signer: false, is_writable: false },
        ];
    
        // Create initialization integration instruction
        let push_amount = 5_000_000;
        let ix = PushBuilder::new()
            .amount(push_amount)
            .controller(controller_pda)
            .authority(authority.pubkey())
            .permission(permission_pda)
            .integration(external_integration_pda)
            .add_remaining_accounts(&external_push_remaining_accounts)
            .instruction();

       let transaction = Transaction::new_signed_with_payer(
           &[ix],
           Some(&authority.pubkey()),
           &[&authority], 
           svm.latest_blockhash(),
       );

       let tx_res = svm.send_transaction(transaction).unwrap_or_else(|e| {
           println!("{:#?}", e);
           panic!();
       });
       println!("{:#?}", tx_res.logs);


       let external_integration_info = svm.get_account(&external_integration_pda).unwrap();
       let external_integration = Integration::try_from_slice(&external_integration_info.data[1..]).unwrap(); // TODO: Fix Discriminator

       let vault_integration_info = svm.get_account(&vault_integration_pda).unwrap();
       let vault_integration = Integration::try_from_slice(&vault_integration_info.data[1..]).unwrap(); // TODO: Fix Discriminator

       let vault_acc = svm.get_account(&vault);
       let recipient_ta_acc = svm.get_account(&receipient_token_account);

       let vault_state = spl_token::state::Account::unpack(&vault_acc.unwrap().data).unwrap();
       let recipient_ta_state = spl_token::state::Account::unpack(&recipient_ta_acc.unwrap().data).unwrap();

        assert_eq!(vault_state.amount, mint_amount - push_amount);
        assert_eq!(recipient_ta_state.amount, push_amount);

        println!("{:?}", vault_state.amount);
        println!("{:?}", recipient_ta_state.amount);

       println!("{:?}", external_integration);
       println!("{:?}", vault_integration);

       assert_eq!(external_integration.controller, controller_pda);
       assert_eq!(external_integration.config, IntegrationConfig::SplTokenExternal(external_config.clone()));
       assert_eq!(external_integration.hash, external_config_hash);



    }





}
