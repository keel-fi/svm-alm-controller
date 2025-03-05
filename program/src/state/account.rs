use pinocchio::pubkey::Pubkey;
use shank::ShankAccount;

#[derive(Clone, Debug, PartialEq, ShankAccount)]
#[repr(C)]
pub struct Account {
    pub some_pubkey: Pubkey,
}
