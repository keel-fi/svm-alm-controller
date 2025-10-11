use pinocchio::account_info::AccountInfo;

/// True when the account is owned by system program and empty with
/// no lamports.
pub fn account_is_uninitialized(acct: &AccountInfo) -> bool {
    acct.data_is_empty() && acct.is_owned_by(&pinocchio_system::ID) && acct.lamports() == 0
}
