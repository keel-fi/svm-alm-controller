#![no_std]

pub mod account_utils;
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

pinocchio_pubkey::declare_id!("ALM1JSnEhc5PkNecbSZotgprBuJujL5objTbwGtpTgTd");
