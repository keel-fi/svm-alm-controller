use svm_alm_controller::state::oracle::OracleConfig;

// Enforces the OracleConfig size doesn't change without tests failing to compile
static_assertions::const_assert_eq!(std::mem::size_of::<OracleConfig>(), 72);
