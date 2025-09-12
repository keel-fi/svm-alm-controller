mod helpers;
mod subs;

use crate::subs::oracle::*;
use helpers::lite_svm_with_programs;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use subs::airdrop_lamports;
use svm_alm_controller_client::generated::accounts::Oracle;

#[cfg(test)]
mod tests {
    use svm_alm_controller_client::generated::types::{ControllerStatus, FeedArgs};
    use switchboard_on_demand::PRECISION;

    use crate::subs::initialize_contoller;

    use super::*;

    #[test_log::test]
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
            0,
            &mint,
        )?;

        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let oracle = oracle.unwrap();
        assert_eq!(oracle.version, 1);
        assert_eq!(oracle.value, 0);
        assert_eq!(oracle.precision, PRECISION);
        assert_eq!(oracle.last_update_slot, 0);
        assert_eq!(oracle.controller, controller_pk);
        assert_eq!(oracle.mint, mint);
        assert_eq!(oracle.reserved, [0; 64]);
        assert_eq!(oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(oracle.feeds[0].price_feed, new_feed);

        // Refresh Oracle account with price.
        refresh_oracle(&mut svm, &authority, &oracle_pda, &new_feed)?;

        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let oracle = oracle.unwrap();
        assert_eq!(oracle.version, 1);
        assert_eq!(oracle.authority, authority.pubkey());
        assert_eq!(oracle.nonce, nonce);
        assert_eq!(oracle.value, update_price);
        assert_eq!(oracle.precision, PRECISION);
        assert_eq!(oracle.last_update_slot, update_slot);
        assert_eq!(oracle.reserved, [0; 64]);
        assert_eq!(oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(oracle.feeds[0].price_feed, new_feed);

        // Update Oracle account with new authority.
        update_oracle(
            &mut svm,
            &controller_pk,
            &authority,
            &oracle_pda,
            &new_feed,
            None, // keep oracle_type unchanged.
            Some(&authority2),
        )?;

        // Verify that only authority is updated.
        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let oracle = oracle.unwrap();
        assert_eq!(oracle.version, 1);
        assert_eq!(oracle.authority, authority2.pubkey());
        assert_eq!(oracle.nonce, nonce);
        assert_eq!(oracle.value, update_price);
        assert_eq!(oracle.precision, PRECISION);
        assert_eq!(oracle.last_update_slot, update_slot);
        assert_eq!(oracle.reserved, [0; 64]);
        assert_eq!(oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(oracle.feeds[0].price_feed, new_feed);

        // Update Oracle account with new feed.
        let new_feed2 = Pubkey::new_unique();
        let update_price = 2_500_000_000_000_000_000; // 2.5 (in 18 precision)
        set_price_feed(&mut svm, &new_feed2, update_price)?;
        update_oracle(
            &mut svm,
            &controller_pk,
            &authority2,
            &oracle_pda,
            &new_feed2,
            Some(FeedArgs { oracle_type }),
            None,
        )?;

        // Verify that feed is updated.
        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let oracle = oracle.unwrap();
        assert_eq!(oracle.version, 1);
        assert_eq!(oracle.authority, authority2.pubkey());
        assert_eq!(oracle.nonce, nonce);
        assert_eq!(oracle.value, 0);
        assert_eq!(oracle.precision, PRECISION);
        assert_eq!(oracle.last_update_slot, 0);
        assert_eq!(oracle.reserved, [0; 64]);
        assert_eq!(oracle.feeds[0].oracle_type, oracle_type);
        assert_eq!(oracle.feeds[0].price_feed, new_feed2);

        Ok(())
    }
}
