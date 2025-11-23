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
pub mod math;
pub mod processor;
pub mod state;
#[cfg(test)]
pub mod unit_test_utils;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

pinocchio_pubkey::declare_id!("ALM1JSnEhc5PkNecbSZotgprBuJujL5objTbwGtpTgTd");
