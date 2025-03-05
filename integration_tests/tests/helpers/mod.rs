use litesvm::LiteSVM;

/// Get LiteSvm with myproject loaded.
pub fn lite_svm_with_programs() -> LiteSVM {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/svm_alm_controller.so");
    svm.add_program(svm_alm_controller_client::programs::SVM_ALM_CONTROLLER_ID, bytes);
    svm
}
