use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{integrations::kamino::derive_anchor_discriminator, KAMINO_LEND_PROGRAM_ID};

pub fn create_refresh_kamino_reserve_instruction(
    reserve: &Pubkey,
    market: &Pubkey,
    scope_prices: &Pubkey,
) -> Instruction {
    let data = derive_anchor_discriminator("global", "refresh_reserve");

    Instruction {
        program_id: KAMINO_LEND_PROGRAM_ID,
        accounts: vec![
            AccountMeta {
                pubkey: *reserve,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *market,
                is_signer: false,
                is_writable: false,
            },
            // pyth oracle
            AccountMeta {
                pubkey: KAMINO_LEND_PROGRAM_ID,
                is_signer: false,
                is_writable: false,
            },
            // switchboard_price_oracle
            AccountMeta {
                pubkey: KAMINO_LEND_PROGRAM_ID,
                is_signer: false,
                is_writable: false,
            },
            // switchboard_twap_oracle
            AccountMeta {
                pubkey: KAMINO_LEND_PROGRAM_ID,
                is_signer: false,
                is_writable: false,
            },
            // scope_prices
            AccountMeta {
                pubkey: *scope_prices,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: data.to_vec(),
    }
}
