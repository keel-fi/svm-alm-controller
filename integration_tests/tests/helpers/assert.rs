use litesvm::types::{FailedTransactionMetadata, TransactionResult};
use solana_sdk::instruction::InstructionError;
use solana_sdk::transaction::TransactionError;
use svm_alm_controller::error::SvmAlmControllerErrors;

pub fn assert_custom_error(
    res: &TransactionResult,
    ix_idx: u8,
    expected_err: SvmAlmControllerErrors,
) {
    let expected_code = expected_err as u32;
    assert!(matches!(
        res,
        Err(FailedTransactionMetadata {
            err: TransactionError::InstructionError(i, InstructionError::Custom(c)),
            ..
        }) if *i == ix_idx && *c == expected_code
    ));
}
