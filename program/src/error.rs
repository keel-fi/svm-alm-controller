use pinocchio::program_error::ProgramError;

/// Errors that may be returned by myproject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SvmAlmControllerErrors {
    // 0
    Invalid,
    // 1
    InvalidPda,
    // 2
    InvalidAccountData,
    // 3
    UnauthorizedAction,
    // 4
    ControllerStatusDoesNotPermitAction,
    // 5
    PermissionStatusDoesNotPermitAction,
    // 6
    IntegrationStatusDoesNotPermitAction,
    // 7
    ReserveStatusDoesNotPermitAction,
    // 8
    StaleOraclePrice,
    // 9
    UnsupportedOracleType,
    // 10
    SwapNotStarted,
    // 11
    SwapHasStarted,
    // 12
    InvalidSwapState,
    // 13
    InvalidInstructions,
    // 14
    SlippageExceeded,
    // 15
    IntegrationHasExpired,
    // 16
    RateLimited,
    // 17
    InvalidControllerAuthority,
    // 18
    InvalidPermission,
    // 19
    InvalidTokenMintExtension,
    // 20
    InvalidAtomicSwapConfiguration,
    // 21
    ControllerDoesNotMatchAccountData,
    // 22
    LZPushInFlight,
    // 23
    InvalidInstructionIndex,
    // 24
    InvalidReserve,
    // 25
    DataNotChangedSinceLastSync,
    // 26
    ControllerFrozen,
}

impl From<SvmAlmControllerErrors> for ProgramError {
    fn from(e: SvmAlmControllerErrors) -> Self {
        ProgramError::Custom(e as u32)
    }
}
