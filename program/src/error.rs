use pinocchio::program_error::ProgramError;

/// Errors that may be returned by myproject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SvmAlmControllerErrors {
    Invalid,
    InvalidPda,
    InvalidEnum,
    InvalidAccountData,
    UnauthorizedAction,
    ControllerStatusDoesNotPermitAction,
    PermissionStatusDoesNotPermitAction,
    IntegrationStatusDoesNotPermitAction,
    ReserveStatusDoesNotPermitAction,
}

impl From<SvmAlmControllerErrors> for ProgramError {
    fn from(e: SvmAlmControllerErrors) -> Self {
        ProgramError::Custom(e as u32)
    }
}
