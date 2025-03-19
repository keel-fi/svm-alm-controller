
use borsh::{maybestd::vec::Vec, BorshSerialize};
use pinocchio::program_error::ProgramError;


#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone,)]
pub struct DepositSingleTokenTypeExactAmountInArgs {
    pub source_token_amount: u64,
    pub minimum_pool_token_amount: u64,
}

impl DepositSingleTokenTypeExactAmountInArgs {

    pub const DISCRIMINATOR: u8 = 4;
    pub const LEN: usize = 17;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized = Vec::with_capacity(1 + Self::LEN);
        serialized.push(Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized).unwrap();
        Ok(serialized)
    } 
    
}



#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone,)]
pub struct WithdrawSingleTokenTypeExactAmountOutArgs {
    pub destination_token_amount: u64,
    pub maximum_pool_token_amount: u64,
}


impl WithdrawSingleTokenTypeExactAmountOutArgs {

    pub const DISCRIMINATOR: u8 = 5;
    pub const LEN: usize = 17;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized = Vec::with_capacity(1 + Self::LEN);
        serialized.push(Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized).unwrap();
        Ok(serialized)
    } 
    
}

