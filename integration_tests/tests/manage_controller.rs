mod helpers;
mod subs;

#[cfg(test)]
mod tests {
     use super::*;
    use solana_sdk::{
        pubkey::Pubkey, signature::Keypair, 
        signer::Signer, 
        transaction::Transaction
    };
    use subs::controller::initialize_contoller;
    use svm_alm_controller::error::SvmAlmControllerErrors;
    use svm_alm_controller_client::generated::{
        instructions::ManageControllerBuilder,  
        types::{ControllerStatus,PermissionStatus,}
    };
    use crate::{
        helpers::{assert::assert_custom_error, lite_svm_with_programs}, 
        subs::{airdrop_lamports, manage_permission}
    };

    #[test]
    fn test_manage_controller_with_invalid_controller_authority_fails() -> Result<(), Box<dyn std::error::Error>> {
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

        // invalid controller authority should throw InvalidControllerAuthority error
        let invalid_controller_authority = Pubkey::new_unique();

        let ixn = ManageControllerBuilder::new()
            .status(ControllerStatus::Active)
            .controller(controller_pk)
            .controller_authority(invalid_controller_authority)
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
}