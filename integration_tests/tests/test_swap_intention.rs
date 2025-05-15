mod helpers;

use std::str::FromStr;

use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;
use helpers::raydium::RAYDIUM_LEGACY_AMM_V4;

fn svm_with_programs() -> LiteSVM {
    let mut svm = LiteSVM::new();

    // Add the CONTROLLER program
    let controller_program_bytes = include_bytes!("../../target/deploy/svm_alm_controller.so");
    svm.add_program(
        svm_alm_controller_client::programs::SVM_ALM_CONTROLLER_ID,
        controller_program_bytes,
    );

    // Add the Orca SWAP program
    let raydium_swap_v4 = include_bytes!("../fixtures/raydium_amm_legacy_v4.so");
    svm.add_program(RAYDIUM_LEGACY_AMM_V4, raydium_swap_v4);

    svm
}


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_swap_through_orca_swap() {

  }
}