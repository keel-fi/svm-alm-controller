use std::error::Error;

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};

use svm_alm_controller::state::AccountDiscriminators;
use svm_alm_controller_client::{generated::accounts::Oracle, SVM_ALM_CONTROLLER_ID};

pub fn set_oracle_price(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    numerator: u64,
    denominator: u64,
) -> Result<(), Box<dyn Error>> {
    let clock: Clock = svm.get_sysvar();
    let oracle = Oracle {
        oracle_type: 0,
        price_feed: Pubkey::new_unique(),
        reserved: [0; 32],
    };

    // TODO: Actually set price in underlying feed.

    // TODO: DRY This up with the generated client stuff
    let mut serialized = Vec::with_capacity(1 + Oracle::LEN);
    serialized.push(AccountDiscriminators::Oracle as u8);
    BorshSerialize::serialize(&oracle, &mut serialized).unwrap();

    svm.set_account(
        *pubkey,
        Account {
            lamports: 1_000_000_000,
            data: serialized,
            owner: SVM_ALM_CONTROLLER_ID,
            executable: false,
            rent_epoch: u64::MAX,
        },
    )?;
    Ok(())
}
