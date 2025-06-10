#[macro_export]
macro_rules! acc_info_as_str {
    ($info:expr) => {
        bs58::encode($info.key()).into_string().as_str()
    };
}

#[macro_export]
macro_rules! key_as_str {
    ($key:expr) => {
        bs58::encode($key).into_string().as_str()
    };
}

/// Defines an account context struct and its `from_accounts` validator.
///
/// ### Example
/// ```ignore
/// define_account_struct! {
///     pub struct AtomicSwapRepay<'info> {
///         payer: signer;
///         controller;
///         authority: signer;
///         integration: mut;
///         token_program: @pubkey(pinocchio_token::ID);
///         reserve: @owner(SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID);
///     }
/// }
/// ```
///
/// ### Supported syntax per field:
/// ```text
/// field: [attr, ...]? [@pubkey(KEY)]? [@owner(KEY1, ...)]?;
/// ```
/// - `signer` - account must be signer
/// - `mut` - account must be writable
/// - `empty` - account data field must be empty
/// - `opt_signer` — account is optional, but must be a signer if provided
/// - `@pubkey(KEY)` — account pubkey must match key provided
/// - `@owner(KEY1, KEY2...)` — account owner must match one of the keys provided
///
/// Use `@remaining_accounts as remaining_accounts;` to capture extra accounts.
///
/// The generated `from_accounts` consumes accounts in order and applies all checks.
#[macro_export]
macro_rules! define_account_struct {
    (
        $vis:vis struct $name:ident < $lt:lifetime > {
            $(
                $field:ident
                $( : $( $attr:ident ),* $(,)? )?
                $( @pubkey ( $check_pubkey:expr ) )?
                $( @owner( $( $check_owner:expr ),+ ) )?
                ;
            )*
            $( @remaining_accounts as $rem_ident:ident ; )?
        }
    ) => {
        $vis struct $name<$lt> {
            $(
                pub $field: & $lt pinocchio::account_info::AccountInfo,
            )*
            $( pub $rem_ident: & $lt [pinocchio::account_info::AccountInfo], )?
        }

        impl<$lt> $name<$lt> {
            pub fn from_accounts(
                accounts: & $lt [pinocchio::account_info::AccountInfo],
            ) -> Result<Self, pinocchio::program_error::ProgramError> {
                use pinocchio::program_error::ProgramError;

                let mut iter = accounts.iter();
                $(
                    let $field = iter.next().ok_or(ProgramError::NotEnoughAccountKeys)?;

                    $(
                        $(
                            if stringify!($attr) == "mut" && !$field.is_writable() {
                                pinocchio_log::log!("{}: invalid mut", stringify!($field));
                                return Err(ProgramError::Immutable);
                            }
                            if stringify!($attr) == "signer" && !$field.is_signer() {
                                pinocchio_log::log!("{}: invalid signer", stringify!($field));
                                return Err(ProgramError::MissingRequiredSignature);
                            }
                            if stringify!($attr) == "empty" && !$field.data_is_empty() {
                                pinocchio_log::log!("{}: not empty", stringify!($field));
                                return Err(ProgramError::AccountAlreadyInitialized);
                            }
                            // Verifies if an optional account is a signer.
                            if stringify!($attr) == "opt_signer" {
                                // Optional account defaults to program_id if not present.
                                if $field.key() != &$crate::ID && !$field.is_signer() {
                                    pinocchio_log::log!("{}: invalid signer", stringify!($field));
                                    return Err(ProgramError::MissingRequiredSignature);
                                }
                            }
                        )*
                    )?

                    $(
                        if $field.key() != &$check_pubkey {
                            pinocchio_log::log!("{}: invalid key", stringify!($field));
                            return Err(ProgramError::IncorrectProgramId);
                        }
                    )?
                    $(
                    if !( $( $field.is_owned_by(&$check_owner) )||+ ) {
                            pinocchio_log::log!("{}: invalid owner", stringify!($field));
                            return Err(ProgramError::InvalidAccountOwner);
                        }
                    )?
                )*

                $( let $rem_ident = iter.as_slice(); )?

                Ok(Self {
                    $(
                        $field,
                    )*
                    $( $rem_ident, )?
                })
            }
        }
    };
}
