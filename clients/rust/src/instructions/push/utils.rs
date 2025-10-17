use solana_instruction::AccountMeta;

use crate::integrations::drift::SpotMarket;

pub fn fetch_inner_remaining_accounts(spot_markets: &[SpotMarket]) -> Vec<AccountMeta> {
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
    oracle_accounts.extend(spot_market_accounts);
    oracle_accounts
}
