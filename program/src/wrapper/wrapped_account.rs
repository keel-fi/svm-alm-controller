use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

/// One trait that every wrapper implements.
///
/// * `Target` is the concrete SPL / PDA struct the wrapper exposes.
/// * `from_account` is the constructor with the same signature you already use.
///
pub trait WrappedAccount<'info>: Sized {
    type Target;
    type Args;

    /// Build the wrapper, performing all invariants checks.
    fn new(info: &'info AccountInfo) -> Result<Self, ProgramError>;

    /// Build the wrapper, performing all invariants checks.
    fn new_with_args(info: &'info AccountInfo, args: Self::Args) -> Result<Self, ProgramError>;

    fn info(&self) -> &'info AccountInfo;
    fn inner(&self) -> &Self::Target;
    fn key(&self) -> &'info Pubkey {
        self.info().key()
    }
}
