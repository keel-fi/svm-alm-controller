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

/// Panic handle that prints the file and line:column numbers.
#[macro_export]
macro_rules! dev_panic_handler {
    () => {
        /// Default panic handler.
        #[cfg(target_os = "solana")]
        #[no_mangle]
        fn custom_panic(info: &core::panic::PanicInfo<'_>) {
            if let Some(location) = info.location() {
                pinocchio_log::log!(
                    "file: {} line {}:{}",
                    location.file(),
                    location.line() as u64,
                    location.column() as u64
                );
            }
            // Panic reporting.
            pinocchio::log::sol_log("** PANICKED **");
        }
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
///         token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
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
/// - `@pubkey(KEY1, KEY2...)` — account pubkey must match one of the keys provided
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
                $( @pubkey( $( $check_pubkey:expr ),+ ) )?
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
                        if !( $( $field.key().eq(&$check_pubkey) )||+ ) {
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

// ============================================================================
// CPI INSTRUCTION MACRO
// ============================================================================
//
// This macro generates type-safe CPI (Cross-Program Invocation) instruction
// structs following the Pinocchio pattern used in token/token22 programs.

/// Generates a CPI instruction struct with automatic AccountMeta construction.
///
/// This macro creates a struct holding account references and implements both
/// `invoke()` and `invoke_signed()` methods for making CPIs to external programs.
/// It follows the Pinocchio pattern for zero-cost, type-safe abstractions.
///
/// # Syntax
///
/// ```ignore
/// cpi_instruction! {
///     [doc comments and attributes]
///     pub struct StructName<'info> {
///         program: PROGRAM_ID,                    // Target program ID
///         discriminator: [u8; 8],                 // Instruction discriminator
///         accounts: {                             // Account declarations
///             account_name: AccountType,
///             other_account: AccountType,
///             ...
///         },
///         remaining_accounts: remaining_accounts_name,  // Optional remaining accounts
///         args: {                                 // Optional instruction arguments
///             arg_name: Type,
///             ...
///         }
///     }
/// }
/// ```
///
/// # Account Types
///
/// | Type | AccountMeta | Description |
/// |------|-------------|-------------|
/// | `Readonly` | `(is_writable: false, is_signer: false)` | Read-only, not a signer |
/// | `Writable` | `(is_writable: true, is_signer: false)` | Writable, not a signer |
/// | `Signer` | `(is_writable: false, is_signer: true)` | Read-only signer |
/// | `Writable<Signer>` | `(is_writable: true, is_signer: true)` | Writable signer |
///
/// # Basic Example (No Arguments)
///
/// ```ignore
/// use crate::cpi_instruction;
///
/// cpi_instruction! {
///     /// Transfer tokens from one account to another
///     pub struct Transfer<'info> {
///         program: spl_token::ID,
///         discriminator: [3, 0, 0, 0, 0, 0, 0, 0],
///         accounts: {
///             from: Writable,           // Source token account
///             to: Writable,             // Destination token account
///             authority: Signer         // Transfer authority
///         }
///     }
/// }
///
/// Usage:
/// Transfer {
///     from: source_account,
///     to: dest_account,
///     authority: owner_account,
/// }
/// .invoke()?;  // No signers needed
///
/// Or with PDA signer:
/// Transfer { from, to, authority }
///     .invoke_signed(&[authority_seeds])?;
/// ```
///
/// # Example with Arguments
///
/// ```ignore
/// cpi_instruction! {
///     /// Initialize a Drift user account
///     pub struct InitializeUser<'info> {
///         program: DRIFT_PROGRAM_ID,
///         discriminator: anchor_discriminator("global", "initialize_user"),
///         accounts: {
///             user: Writable<Signer>,
///             user_stats: Writable,
///             state: Writable,
///             authority: Signer,
///             payer: Writable<Signer>,
///             system_program: Readonly
///         },
///         args: {
///             sub_account_id: u16,       // Arguments must implement BorshSerialize
///             name: [u8; 32]
///         }
///     }
/// }
///
/// # Example with Remaining Accounts
///
/// ```ignore
/// cpi_instruction! {
///     /// Deposit tokens into a Drift spot market
///     pub struct Deposit<'info> {
///         program: DRIFT_PROGRAM_ID,
///         discriminator: anchor_discriminator("global", "deposit"),
///         accounts: {
///             state: Readonly,
///             user: Writable,
///             user_stats: Writable,
///             authority: Signer,
///             spot_market_vault: Writable,
///             user_token_account: Writable,
///             token_program: Readonly
///         },
///         remaining_accounts: remaining_accounts,  // Additional accounts for complex instructions
///         args: {
///             market_index: u16,
///             amount: u64,
///         }
///     }
/// }
/// ```
///
/// # Instruction Data Serialization
///
/// - **No arguments**: Data is just the discriminator
/// - **With arguments**: Discriminator followed by Borsh-serialized arguments
///
/// Arguments are serialized in the order they appear in the macro definition.
///
/// # Multiple Signers
///
/// The macro supports multiple PDA signers:
///
/// ```ignore
/// InitializeUser { ... }
///     .invoke_signed(&[
///         signer_seeds_1,
///         signer_seeds_2,
///     ])?;
/// ```
///
/// # Account Documentation
///
/// You can add doc comments to individual accounts:
///
/// ```ignore
/// cpi_instruction! {
///     pub struct Example<'info> {
///         program: PROGRAM_ID,
///         discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
///         accounts: {
///             /// The source account for funds
///             source: Writable,
///             
///             /// The destination account
///             destination: Writable
///         }
///     }
/// }
/// ```
///
/// # Internal Implementation
///
/// The macro uses internal "matcher" rules prefixed with `@`:
///
/// - `@meta`: Converts account types to AccountMeta
/// - `@data`: Serializes instruction data (discriminator + args)
///
/// These are implementation details and should not be called directly.
///
/// All generated functions are marked `#[inline(always)]` where appropriate,
/// allowing the compiler to optimize away any abstraction overhead.
#[macro_export]
macro_rules! cpi_instruction {
    // ========================================================================
    // UNIFIED MACRO RULE: HANDLES ALL COMBINATIONS
    // ========================================================================
    // This rule handles all combinations: with/without args, with/without remaining_accounts
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident<$lifetime:lifetime> {
            program: $program_id:expr,
            discriminator: $discriminator:expr,
            accounts: {
                $(
                    $(#[doc = $doc:expr])*
                    $account_name:ident: $account_type:ident $(<$($modifier:ident),+>)?
                ),* $(,)?
            }
            $(, remaining_accounts: $remaining_accounts_name:ident)?
            $(, args: {
                $(
                    $arg_name:ident: $arg_type:ty
                ),* $(,)?
            })?
        }
    ) => {
        // ====================================================================
        // STRUCT GENERATION
        // ====================================================================
        $(#[$meta])*
        $vis struct $name<$lifetime> {
            $(
                $(#[doc = $doc])*
                pub $account_name: &$lifetime pinocchio::account_info::AccountInfo,
            )*
            $(
                pub $remaining_accounts_name: &$lifetime [pinocchio::account_info::AccountInfo],
            )?
            $($(
                pub $arg_name: $arg_type,
            )*)?
        }

        // ====================================================================
        // IMPL BLOCK GENERATION
        // ====================================================================
        impl<$lifetime> $name<$lifetime> {
            pub const DISCRIMINATOR: &'static [u8] = &$discriminator;

            #[inline(always)]
            pub fn invoke(&self) -> pinocchio::ProgramResult {
                self.invoke_signed(&[])
            }

            #[inline(always)]
            pub fn invoke_signed(&self, signers: &[pinocchio::instruction::Signer]) -> pinocchio::ProgramResult {
                extern crate alloc;
                use alloc::vec::Vec;

                let base_accounts = [
                    $(
                        cpi_instruction!(@meta $account_name: $account_type $(<$($modifier),+>)?, self),
                    )*
                ];

                // Handle remaining accounts if present
                let mut accounts = Vec::with_capacity(base_accounts.len() + $(self.$remaining_accounts_name.len() +)? 0);
                accounts.extend_from_slice(&base_accounts);
                $(
                    for account in self.$remaining_accounts_name {
                        accounts.push(pinocchio::instruction::AccountMeta::new(account.key(), account.is_writable(), account.is_signer()));
                    }
                )?

                let base_account_infos = [
                    $(self.$account_name,)*
                ];
                let mut account_infos = Vec::with_capacity(base_account_infos.len() + $(self.$remaining_accounts_name.len() +)? 0);
                account_infos.extend_from_slice(&base_account_infos);
                $(
                    account_infos.extend(self.$remaining_accounts_name.iter());
                )?

                let data = cpi_instruction!(@data self, Self::DISCRIMINATOR $(, $($arg_name),*)?);

                let ix = pinocchio::instruction::Instruction {
                    program_id: &$program_id,
                    accounts: &accounts,
                    data: &data,
                };

                pinocchio::program::slice_invoke_signed(&ix, &account_infos, signers)
            }
        }
    };

    // ========================================================================
    // INTERNAL MATCHER: @meta (Account Type → AccountMeta)
    // ========================================================================
    // These rules convert high-level account types (Readonly, Writable, etc.)
    // into AccountMeta structs with the correct is_writable and is_signer flags.
    //
    // The @ prefix indicates these are internal implementation details and
    // should not be called directly by users of the macro.

    // Writable + Signer: Account that can be modified and must sign
    // Used for: Payer accounts, initializing accounts that sign themselves
    // AccountMeta::new(pubkey, is_writable: true, is_signer: true)
    (@meta $name:ident: Writable<Signer>, $self:ident) => {
        pinocchio::instruction::AccountMeta::new($self.$name.key(), true, true)
    };

    // Writable: Account that can be modified but doesn't sign
    // Used for: Token accounts being debited/credited, state accounts
    // AccountMeta::new(pubkey, is_writable: true, is_signer: false)
    (@meta $name:ident: Writable, $self:ident) => {
        pinocchio::instruction::AccountMeta::new($self.$name.key(), true, false)
    };

    // Signer: Account that must sign but won't be modified
    // Used for: Authority accounts that approve actions
    // AccountMeta::new(pubkey, is_writable: false, is_signer: true)
    (@meta $name:ident: Signer, $self:ident) => {
        pinocchio::instruction::AccountMeta::new($self.$name.key(), false, true)
    };

    // Readonly: Account that neither signs nor gets modified
    // Used for: Program IDs, reference accounts, sysvars
    // AccountMeta::new(pubkey, is_writable: false, is_signer: false)
    (@meta $name:ident: Readonly, $self:ident) => {
        pinocchio::instruction::AccountMeta::new($self.$name.key(), false, false)
    };

    // ========================================================================
    // INTERNAL MATCHER: @data (Instruction Data Serialization)
    // ========================================================================
    // These rules handle serializing the instruction data, which consists of:
    // 1. The discriminator (8 bytes)
    // 2. Optional arguments serialized using Borsh
    //
    // Pattern matching determines whether arguments are present and handles
    // both cases appropriately.

    // No arguments: instruction data is just the discriminator
    // This is used for simple instructions like close_account, sync_native, etc.
    (@data $self:ident, $discriminator:expr) => {
        $discriminator
    };

    // Empty arguments case: when $($arg_name),* is empty
    (@data $self:ident, $discriminator:expr, ) => {
        $discriminator
    };

    // With arguments: discriminator followed by Borsh-serialized args
    // Each argument is serialized in order and appended to the data vector.
    //
    // Example generated code:
    // {
    //     use borsh::BorshSerialize;
    //     let mut data = [1, 2, 3, 4, 5, 6, 7, 8].to_vec();
    //     self.amount.serialize(&mut data).unwrap();
    //     self.decimals.serialize(&mut data).unwrap();
    //     data
    // }
    (@data $self:ident, $discriminator:expr, $($arg_name:ident),+) => {{
        use borsh::BorshSerialize;
        let mut data = $discriminator.to_vec();
        $(
            // Serialize each argument in the order they were declared
            // unwrap() is safe because serialization to Vec<u8> shouldn't fail
            $self.$arg_name.serialize(&mut data).unwrap();
        )+
        data
    }};


}
