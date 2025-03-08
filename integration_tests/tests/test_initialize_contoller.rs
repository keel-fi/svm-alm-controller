use helpers::lite_svm_with_programs;
use svm_alm_controller_client::instructions::{InitializeControllerBuilder, ManagePermissionBuilder};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};
use solana_sdk::pubkey::Pubkey;
use svm_alm_controller_client::programs::SVM_ALM_CONTROLLER_ID;
use solana_sdk::system_program;

use borsh::BorshDeserialize;
use svm_alm_controller_client::{accounts::{Controller, Permission}, types::{ControllerStatus,PermissionStatus}};

mod helpers;

#[cfg(test)]
mod tests {



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
    
    
        // Create initialization instruction with status 1 (Active)
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
            &[&authority],  // Both payer and authority need to sign
            svm.latest_blockhash(),
        );

        let tx_res = svm.send_transaction(transaction).unwrap();

        let controller_info = svm.get_account(&controller_pda).unwrap();
        let permission_info = svm.get_account(&permission_pda).unwrap();

    
        println!("{:?}", controller_info.data);
        println!("{:?}", permission_info.data);

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
    
    
        // Create initialization instruction with status 1 (Active)
        let ix = ManagePermissionBuilder::new()
            .status(PermissionStatus::Active) 
            .can_execute_swap(true)
            .can_manage_permissions(true)
            .can_invoke_external_transfer(true)
            .can_reallocate(true)
            .can_freeze(true)
            .can_unfreeze(true)
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


        println!("{:?}", controller);
        println!("{:?}", new_permission);
        println!("{:?}", permission);

        assert_eq!(new_permission.authority, new_authority.pubkey());
        assert_eq!(new_permission.controller, controller_pda);
        assert_eq!(new_permission.status, PermissionStatus::Active);
        assert_eq!(new_permission.can_execute_swap, true);
        assert_eq!(new_permission.can_freeze, true);
        assert_eq!(new_permission.can_manage_permissions, true);
        assert_eq!(new_permission.can_reallocate, true);
        assert_eq!(new_permission.can_invoke_external_transfer, true);
        assert_eq!(new_permission.can_unfreeze, true);


        // MANAGE_PERMISSION - update existing (own) permissions


        // Create initialization instruction with status 1 (Active)
        let ix = ManagePermissionBuilder::new()
            .status(PermissionStatus::Active) 
            .can_execute_swap(true)
            .can_manage_permissions(true)
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


        println!("{:?}", controller);
        println!("{:?}", permission);

        assert_eq!(permission.authority, authority.pubkey());
        assert_eq!(permission.controller, controller_pda);
        assert_eq!(permission.status, PermissionStatus::Active);
        assert_eq!(permission.can_execute_swap, true);
        assert_eq!(permission.can_freeze, true);
        assert_eq!(permission.can_manage_permissions, true);
        assert_eq!(permission.can_reallocate, true);
        assert_eq!(permission.can_invoke_external_transfer, true);
        assert_eq!(permission.can_unfreeze, true);

    }



}
