use solana_instruction::AccountMeta;

use crate::integrations::drift::SpotMarket;

/// Get the inner remaining accounts for the drift push instruction.
///
/// # Arguments
///
/// * `spot_markets` - The spot markets to get the inner remaining accounts for.
///
/// # Returns
///
/// A vector of `AccountMeta` for the inner remaining accounts.
pub fn get_inner_remaining_accounts(spot_markets: &[SpotMarket]) -> Vec<AccountMeta> {
    let mut oracle_accounts = vec![];
    let mut spot_market_accounts = vec![];
    for spot_market in spot_markets {
        oracle_accounts.push(AccountMeta {
            pubkey: spot_market.oracle,
            is_signer: false,
            is_writable: false,
        });
        spot_market_accounts.push(AccountMeta {
            pubkey: spot_market.pubkey,
            is_signer: false,
            is_writable: true,
        });
    }
    // Drift requires the oracle accounts to be the first, then spot, then perps.
    let remaining_accounts = [oracle_accounts, spot_market_accounts].concat();
    remaining_accounts
}

/// Extract oracle and insurance fund addresses from spot market account data
pub fn extract_spot_market_data(
    spot_market_account_data: &[u8],
) -> Result<SpotMarket, Box<dyn std::error::Error>> {
    // Skip the discriminator
    if spot_market_account_data[..SpotMarket::DISCRIMINATOR.len()] != SpotMarket::DISCRIMINATOR {
        return Err("Invalid discriminator".into());
    }

    let market_data = &spot_market_account_data[SpotMarket::DISCRIMINATOR.len()..];

    // Parse the SpotMarket struct using bytemuck
    let spot_market = bytemuck::try_from_bytes::<SpotMarket>(market_data)
        .map_err(|e| format!("Failed to parse spot market data: {}", e))?;

    Ok(*spot_market)
}
