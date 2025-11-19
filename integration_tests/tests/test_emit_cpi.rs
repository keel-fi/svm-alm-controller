mod helpers;
mod subs;

use helpers::lite_svm_with_programs;
use subs::airdrop_lamports;

#[cfg(test)]
mod tests {

    use solana_sdk::{
        native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair, signer::Signer,
        system_instruction, transaction::Transaction,
    };
    use svm_alm_controller_client::generated::{
        instructions::EmitEventBuilder, programs::SVM_ALM_CONTROLLER_ID,
    };

    use super::*;

    #[test]
    fn test_malicious_emit_cpi_via_keypair() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let payer = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &payer.pubkey(), 1_000_000_000_000)?;

        let bad_authority = Keypair::new();

        // add instruction to that sets owner
        let create_acct_ix = system_instruction::create_account(
            &payer.pubkey(),
            &bad_authority.pubkey(),
            LAMPORTS_PER_SOL,
            0,
            &Pubkey::from(SVM_ALM_CONTROLLER_ID),
        );

        // add EmitCpi instruction
        let emit_cpi_ix = EmitEventBuilder::new()
            .authority(bad_authority.pubkey())
            .controller_id([0u8; 2])
            .data(vec![1, 2, 3, 4, 5, 6])
            .instruction();

        // Send TX and expect it to fail
        let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
            &[create_acct_ix, emit_cpi_ix],
            Some(&payer.pubkey()),
            &[&bad_authority, &payer],
            svm.latest_blockhash(),
        ));

        assert!(tx_result.is_err());

        Ok(())
    }
}
