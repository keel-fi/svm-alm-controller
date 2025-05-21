mod helpers;
mod subs;

use borsh::BorshDeserialize;
use helpers::lite_svm_with_programs;
use litesvm::LiteSVM;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, transaction::Transaction,
};
use std::error::Error;
use subs::airdrop_lamports;
use svm_alm_controller::processor::InitializeOracle;
use svm_alm_controller_client::generated::{
    accounts::Oracle, instructions::InitializeOracleBuilder, programs::SVM_ALM_CONTROLLER_ID,
};

pub fn derive_oracle_pda(feed: &Pubkey) -> Pubkey {
    let (controller_pda, _controller_bump) = Pubkey::find_program_address(
        &[b"oracle", &feed.to_bytes()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    controller_pda
}

pub fn fetch_oracle_account(
    svm: &LiteSVM,
    oracle_pda: &Pubkey,
) -> Result<Option<Oracle>, Box<dyn Error>> {
    let oracle_info = svm.get_account(oracle_pda);
    match oracle_info {
        Some(info) => {
            if info.data.is_empty() {
                Ok(None)
            } else {
                Oracle::try_from_slice(&info.data[1..])
                    .map(Some)
                    .map_err(Into::into)
            }
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test_log::test]
    fn initialize_oracle() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;

        let new_feed = Pubkey::new_unique();
        let oracle_pda = derive_oracle_pda(&new_feed);
        let oracle_type = 0;

        let ixn = InitializeOracleBuilder::new()
            .oracle(oracle_pda)
            .price_feed(new_feed)
            .system_program(system_program::ID)
            .payer(authority.pubkey())
            .oracle_type(oracle_type)
            .instruction();

        let txn = Transaction::new_signed_with_payer(
            &[ixn],
            Some(&authority.pubkey()),
            &[&authority],
            svm.latest_blockhash(),
        );
        let tx_result = svm.send_transaction(txn);
        assert!(tx_result.is_ok(), "Transaction failed to execute");

        let oracle: Option<Oracle> = fetch_oracle_account(&svm, &oracle_pda)?;
        assert!(oracle.is_some(), "Oracle account is not found");
        let oracle = oracle.unwrap();
        assert_eq!(oracle.oracle_type, oracle_type);
        assert_eq!(oracle.price_feed, new_feed);

        Ok(())
    }
}
