use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{integrations::utils::anchor_discriminator, KAMINO_LEND_PROGRAM_ID};

/// If obligation has reserves, they need to be added as remaining accounts.
/// for the sake of simplicity, this method only support obligations with 1 reserve.
/// TODO: add support for more
pub fn create_refresh_kamino_obligation_instruction(
    market: &Pubkey,
    obligation: &Pubkey,
    reserves: Vec<&Pubkey>,
) -> Instruction {
    let data = anchor_discriminator("global", "refresh_obligation");

    let mut accounts = vec![
        AccountMeta {
            pubkey: *market,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *obligation,
            is_signer: false,
            is_writable: true,
        },
    ];

    accounts.extend(reserves.into_iter().map(|reserve| AccountMeta {
        pubkey: *reserve,
        is_signer: false,
        is_writable: true,
    }));

    Instruction {
        program_id: KAMINO_LEND_PROGRAM_ID,
        accounts: accounts,
        data: data.to_vec(),
    }
}
