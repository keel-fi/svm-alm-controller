#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
    use svm_alm_controller_client::generated::types::{ControllerStatus, PermissionStatus};

    use crate::subs::{initialize_contoller, manage_permission, oracle::set_oracle_price};

    fn lite_svm_with_programs() -> LiteSVM {
        let mut svm = LiteSVM::new();

        // Add the CONTROLLER program
        let controller_program_bytes =
            include_bytes!("../../../target/deploy/svm_alm_controller.so");
        svm.add_program(
            svm_alm_controller_client::SVM_ALM_CONTROLLER_ID,
            controller_program_bytes,
        );

        svm
    }

    #[test_log::test]
    fn test_happy_path_initialize_swap_pair() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();
        let oracle_pubkey = Pubkey::new_unique();
        set_oracle_price(&mut svm, &oracle_pubkey, 1_000_000_000, 1_000_000_000)?;

        let relayer_authority_kp = Keypair::new();
        svm.airdrop(&relayer_authority_kp.pubkey(), 100_000_000)
            .unwrap();

        // Set up a controller and relayer with swap capabilities.
        let (controller_pk, _authority_permission_pk) = initialize_contoller(
            &mut svm,
            &relayer_authority_kp,
            &relayer_authority_kp,
            ControllerStatus::Active,
            321u16, // Id
        )?;
        let _ = manage_permission(
            &mut svm,
            &controller_pk,
            &relayer_authority_kp,          // payer
            &relayer_authority_kp,          // calling authority
            &relayer_authority_kp.pubkey(), // subject authority
            PermissionStatus::Active,
            false, // can_execute_swap,
            false, // can_manage_permissions,
            false, // can_invoke_external_transfer,
            false, // can_reallocate,
            false, // can_freeze,
            false, // can_unfreeze,
            true,  // can_manage_integrations
        )?;
        // TODO: Initialize the SwapPair

        // TODO: Assert the SwapPair was created
        Ok(())
    }
}
