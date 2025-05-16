use std::error::Error;

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};

use svm_alm_controller::state::AccountDiscriminators;
use svm_alm_controller_client::{
    generated::{
        accounts::Oracle,
        types::{OracleConfig, PythConfig, PythTransformation},
    },
    SVM_ALM_CONTROLLER_ID,
};

pub fn set_oracle_price(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    numerator: u64,
    denominator: u64,
) -> Result<(), Box<dyn Error>> {
    let clock: Clock = svm.get_sysvar();
    let oracle = Oracle {
        last_updated_block: clock.slot,
        price_numerator: numerator,
        price_denominator: denominator,
        config: OracleConfig::PythFeed(PythConfig {
            feed_id: [0u8; 32],
            min_signatures: 0,
            max_staleness_seconds: 0,
            guardian_set: Pubkey::new_unique(),
            transformations: [
                PythTransformation::None,
                PythTransformation::None,
                PythTransformation::None,
            ],
        }),
    };

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
