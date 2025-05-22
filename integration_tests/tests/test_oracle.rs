mod helpers;
mod subs;

use crate::subs::oracle::*;
use helpers::lite_svm_with_programs;
use litesvm::LiteSVM;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, transaction::Transaction,
};
use std::error::Error;
use subs::airdrop_lamports;
use svm_alm_controller::processor::InitializeOracle;
use svm_alm_controller_client::generated::{
    accounts::Oracle,
    instructions::{InitializeOracleBuilder, RefreshOracleBuilder},
    programs::SVM_ALM_CONTROLLER_ID,
};

#[cfg(test)]
mod tests {
    use switchboard_on_demand::PRECISION;

    use super::*;

    #[test_log::test]
    fn initialize_and_refresh_oracle() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        let nonce = Pubkey::new_unique();
        let new_feed = Pubkey::new_unique();
        let oracle_pda = derive_oracle_pda(&nonce);
        let oracle_type = 0;

        // Stub price feed data
        let update_slot = 1000_000;
        let update_price = 1_000_000_000;
        svm.warp_to_slot(update_slot);
        set_price_feed(&mut svm, &new_feed, update_price)?;

        // Initialize Oracle account
        initalize_oracle(&mut svm, &authority, &nonce, &new_feed, 0)?;

        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let oracle = oracle.unwrap();
        assert_eq!(oracle.oracle_type, oracle_type);
        assert_eq!(oracle.price_feed, new_feed);
        assert_eq!(oracle.value, 0);
        assert_eq!(oracle.precision, 0);
        assert_eq!(oracle.last_update_slot, 0);
        assert_eq!(oracle.reserved, [0; 64]);

        // Refresh Oracle account with price.
        refresh_oracle(&mut svm, &authority, &oracle_pda, &new_feed)?;

        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let oracle = oracle.unwrap();
        assert_eq!(oracle.oracle_type, oracle_type);
        assert_eq!(oracle.authority, authority.pubkey());
        assert_eq!(oracle.nonce, nonce);
        assert_eq!(oracle.price_feed, new_feed);
        assert_eq!(oracle.value, update_price);
        assert_eq!(oracle.precision, PRECISION);
        assert_eq!(oracle.last_update_slot, update_slot);
        assert_eq!(oracle.reserved, [0; 64]);

        Ok(())
    }
}
