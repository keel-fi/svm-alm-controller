#![no_std]

pub mod error;
#[cfg(feature = "idl")]
pub mod instructions;
pub mod processor;
pub mod state;
pub mod constants;
pub mod macros;
pub mod enums;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

// TODO update with your program ID
pinocchio_pubkey::declare_id!("2NCpU9nsgLfhqKX5CDVz24FfsZ8aRwjgUWtFbPsVZVu2");
