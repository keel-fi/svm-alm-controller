use borsh::{BorshDeserialize, BorshSerialize};
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct DriftConfig {
    // The sub account where borrow/lend are cross collateralized
    pub sub_account_id: u16,
    // Spot market to deposit into (mint specific)
    // Indexes can be seen here: https://github.com/drift-labs/protocol-v2/blob/master/sdk/src/constants/spotMarkets.ts
    pub spot_market_index: u16,
    pub _padding: [u8; 220],
}
