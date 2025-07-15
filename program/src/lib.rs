#![no_std]

pub mod constants;
pub mod enums;
pub mod error;
pub mod events;
#[cfg(feature = "idl")]
pub mod instructions;
pub mod instructions;
pub mod integrations;
pub mod macros;
pub mod processor;
pub mod state;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

// TODO update with your program ID
pinocchio_pubkey::declare_id!("98BiSW5kL3nfgGeoLmYi85EAgabcdNhmXPwJ9Yc8w3sD");
