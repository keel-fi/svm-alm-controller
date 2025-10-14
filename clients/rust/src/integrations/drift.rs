use solana_pubkey::{pubkey, Pubkey};

pub const DRIFT_PROGRAM_ID: Pubkey = pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

/// Derives State PDA
pub fn derive_state_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"drift_state"], &DRIFT_PROGRAM_ID).0
}

/// Derives UserStats PDA
pub fn derive_user_stats_pda(authority: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user_stats", authority.as_ref()], &DRIFT_PROGRAM_ID).0
}

/// Derives User subaccount PDA
pub fn derive_user_pda(authority: &Pubkey, sub_account_id: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"user",
            authority.as_ref(),
            sub_account_id.to_le_bytes().as_ref(),
        ],
        &DRIFT_PROGRAM_ID,
    )
    .0
}

/// Derives SpotMarket PDA
pub fn derive_spot_market_pda(market_index: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[b"spot_market", market_index.to_le_bytes().as_ref()],
        &DRIFT_PROGRAM_ID,
    )
    .0
}

/// Derives SpotMarket Vault PDA
pub fn derive_spot_market_vault_pda(market_index: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[b"spot_market_vault", market_index.to_le_bytes().as_ref()],
        &DRIFT_PROGRAM_ID,
    )
    .0
}
