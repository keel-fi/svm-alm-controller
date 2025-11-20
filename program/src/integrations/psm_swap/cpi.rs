use crate::{cpi_instruction, integrations::psm_swap::constants::PSM_SWAP_PROGRAM_ID};

const ADD_LIQUIDITY_DISCRIMINATOR: [u8; 1] = [6];
const REMOVE_LIQUIDITY_DISCRIMINATOR: [u8; 1] = [7];

cpi_instruction! {
    pub struct AddLiquidityToPsmToken<'info> {
        program: PSM_SWAP_PROGRAM_ID,
        discriminator: ADD_LIQUIDITY_DISCRIMINATOR,
        accounts: {
            payer: Signer,
            psm_pool: Readonly,
            psm_token: Readonly,
            mint: Readonly,
            token_vault: Writable,
            user_token_account: Writable,
            token_program: Readonly,
            associated_token_program: Readonly
        },
        args: {
            amount: u64
        }
    }
}

cpi_instruction! {
    pub struct RemoveLiquidityFromPsmToken<'info> {
        program: PSM_SWAP_PROGRAM_ID,
        discriminator: REMOVE_LIQUIDITY_DISCRIMINATOR,
        accounts: {
            liquidity_owner: Signer,
            psm_pool: Readonly,
            psm_token: Readonly,
            mint: Readonly,
            token_vault: Writable,
            owner_token_account: Writable,
            token_program: Readonly,
            associated_token_program: Readonly
        },
        args: {
            amount: u64
        }
    }
}
