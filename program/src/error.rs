use pinocchio::program_error::ProgramError;

/// Errors that may be returned by myproject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SvmAlmControllerErrors {
    // 0
    Invalid,
    // 1
    InvalidPda,
    // 2
    InvalidEnum,
    // 3
    InvalidAccountData,
    // 4
    UnauthorizedAction,
    // 5
    ControllerStatusDoesNotPermitAction,
    // 6
    PermissionStatusDoesNotPermitAction,
    // 7
    IntegrationStatusDoesNotPermitAction,
    // 8
    ReserveStatusDoesNotPermitAction,
    // 9
    StaleOraclePrice,
    // 10
    UnsupportedOracleType,
    // 11
    SwapNotStarted,
    // 12
    SwapHasStarted,
    // 13
    InvalidSwapState,
    // 14
    InvalidInstructions,
    // 15
    SlippageExceeded,
    // 16
    IntegrationHasExpired,
    // 17
    RateLimited,
    // 18
    InvalidControllerAuthority,
    // 19,
    InvalidPermission,
    // 20,
    InvalidTokenMintExtension,
    // 21.
    InvalidAtomicSwapConfiguration,
    // 22.
    ControllerDoesNotMatchAccountData,
    // 23.
    LZPushInFlight,
    // 24.
    InvalidInstructionIndex,
    // 25.
    InvalidReserve,
    // 26.
    DataNotChangedSinceLastSync,
}

impl From<SvmAlmControllerErrors> for ProgramError {
    fn from(e: SvmAlmControllerErrors) -> Self {
        ProgramError::Custom(e as u32)
    }
}
