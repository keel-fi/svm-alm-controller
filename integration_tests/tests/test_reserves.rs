mod helpers;
mod subs;
use crate::helpers::constants::USDC_TOKEN_MINT_PUBKEY;
use crate::subs::{controller::manage_controller, initialize_reserve};
use helpers::{assert::assert_custom_error, setup_test_controller, TestContext};
use solana_sdk::signer::Signer;
use svm_alm_controller::error::SvmAlmControllerErrors;
use svm_alm_controller_client::generated::types::{ControllerStatus, ReserveStatus};

#[cfg(test)]
mod tests {

    use solana_sdk::transaction::Transaction;
    use svm_alm_controller_client::{
        create_initialize_reserve_instruction, create_manage_reserve_instruction,
        create_sync_reserve_instruction,
    };

    use super::*;

    #[test]
    fn test_initialize_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        let instruction = create_initialize_reserve_instruction(
            &super_authority.pubkey(),
            &controller_pk,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
            ReserveStatus::Active,
            1_000_000_000_000,
            1_000_000_000_000,
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );

        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test]
    fn test_manage_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Initialize reserve first (while controller is active)
        let _reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            0,
            0,
            &spl_token::ID,
        )?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        let instruction = create_manage_reserve_instruction(
            &controller_pk,
            &super_authority.pubkey(),
            &USDC_TOKEN_MINT_PUBKEY,
            ReserveStatus::Suspended,
            1000,
            2000,
        );

        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }

    #[test]
    fn test_sync_reserve_fails_when_frozen() -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Initialize reserve first (while controller is active)
        let _reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &super_authority,
            &super_authority,
            ReserveStatus::Active,
            0,
            0,
            &spl_token::ID,
        )?;

        // Freeze the controller
        manage_controller(
            &mut svm,
            &controller_pk,
            &super_authority, // payer
            &super_authority, // calling authority
            ControllerStatus::Frozen,
        )?;

        let instruction = create_sync_reserve_instruction(
            &controller_pk,
            &USDC_TOKEN_MINT_PUBKEY,
            &spl_token::ID,
        );
        let txn = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&super_authority.pubkey()),
            &[&super_authority, &super_authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);

        assert_custom_error(&tx_result, 0, SvmAlmControllerErrors::ControllerFrozen);

        Ok(())
    }
}
