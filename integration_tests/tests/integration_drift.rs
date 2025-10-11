mod helpers;
mod subs;

#[cfg(test)]
mod tests {

    use crate::{
        helpers::{setup_test_controller, TestContext},
        subs::{initialize_mint, initialize_reserve},
    };
    use solana_sdk::{pubkey::Pubkey, signer::Signer};
    use svm_alm_controller_client::generated::types::ReserveStatus;
    use test_case::test_case;

    #[test_case(spl_token::ID ; "SPL Token")]
    #[test_case(spl_token_2022::ID ; "Token2022")]
    fn initiailize_drift_success(token_program: Pubkey) -> Result<(), Box<dyn std::error::Error>> {
        let TestContext {
            mut svm,
            controller_pk,
            super_authority,
        } = setup_test_controller()?;

        // Initialize a mint
        let mint = initialize_mint(
            &mut svm,
            &super_authority,
            &super_authority.pubkey(),
            None,
            6,
            None,
            &token_program,
            None,
        )?;

        // Initialize a reserve for the token
        let _reserve_keys = initialize_reserve(
            &mut svm,
            &controller_pk,
            &mint,            // mint
            &super_authority, // payer
            &super_authority, // authority
            ReserveStatus::Active,
            1_000_000_000, // rate_limit_slope
            1_000_000_000, // rate_limit_max_outflow
            &token_program,
        )?;

        // TODO initialize Drift Integration

        Ok(())
    }
}
