use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

pub trait LendingBalanceFromProtocolState<T> {
    /// Returns the balance of tokens that the Controller has a claim on.
    /// This uses protocol-specific logic to calculate the balance
    /// from the protocol state.
    fn get_lending_balance(input: T) -> u64;
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct LendingState {
    /// The balance of tokens that the Controller has a claim on.
    /// This includes the deposit amount plus any interest earned.
    pub balance: u64,
    /// Padding
    pub _padding: [u8; 40],
}
