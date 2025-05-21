mod helpers;
mod subs;

use helpers::lite_svm_with_programs;
use subs::airdrop_lamports;

use solana_sdk::{signature::Keypair, signer::Signer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test]
    fn initialize_oracle() -> Result<(), Box<dyn std::error::Error>> {
        let mut svm = lite_svm_with_programs();

        let authority = Keypair::new();

        // Airdrop to payer
        airdrop_lamports(&mut svm, &authority.pubkey(), 1_000_000_000)?;
        Ok(())
    }
}
