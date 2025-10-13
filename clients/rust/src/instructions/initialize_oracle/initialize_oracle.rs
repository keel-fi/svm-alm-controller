use crate::{
    derive_controller_authority_pda, derive_oracle_pda,
    generated::instructions::InitializeOracleBuilder,
};
use solana_instruction::Instruction;
use solana_program::system_program;
use solana_pubkey::Pubkey;

pub fn create_initialize_oracle_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    nonce: &Pubkey,
    price_feed: &Pubkey,
    oracle_type: u8,
    mint: &Pubkey,
    quote_mint: &Pubkey,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);
    let oracle_pda = derive_oracle_pda(&nonce);
    InitializeOracleBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .oracle(oracle_pda)
        .price_feed(*price_feed)
        .system_program(system_program::ID)
        .payer(*authority)
        .oracle_type(oracle_type)
        .nonce(*nonce)
        .base_mint(*mint)
        .quote_mint(*quote_mint)
        .instruction()
}
