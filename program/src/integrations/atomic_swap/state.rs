use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct AtomicSwapState {
    // Amount of token a in reserve before borrow step in atomic swap.
    pub last_balance_a: u64,
    // Amount of token b in reserve before borrow step in atomic swap.
    pub last_balance_b: u64,
    // Amount of token a borrowed
    pub amount_borrowed: u64,
    // Recipient's token a account balance before borrow.
    pub recipient_token_a_pre: u64,
    // Recipient's token b account balance before borrow.
    pub recipient_token_b_pre: u64,
    // If additional tokens in recipient's token account balance (compared to
    // recipient_token_a_pre should be repaid. Note that this is for accounting
    // purposes and that is no enforcement that the same token account has to be used.
    pub repay_excess_token_a: bool,
    pub _padding: [u8; 7],
}

impl AtomicSwapState {
    pub fn has_swap_started(&self) -> bool {
        self.amount_borrowed > 0
    }

    pub fn reset(&mut self) {
        self.last_balance_a = 0;
        self.last_balance_b = 0;
        self.amount_borrowed = 0;
        self.recipient_token_a_pre = 0;
        self.recipient_token_b_pre = 0;
        self.repay_excess_token_a = false;
    }
}
