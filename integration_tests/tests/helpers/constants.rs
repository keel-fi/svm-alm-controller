#![allow(dead_code)]

use solana_sdk::{pubkey, pubkey::Pubkey};

// CCTP
pub const CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID: Pubkey =
    pubkey!("CCTPmbSD7gX1bxKPAmg77w8oFzNFpaQiQUWD43TKaecd");
pub const CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID: Pubkey =
    pubkey!("CCTPiPYPc6AsJuwueEnWgSgucamXDZwBd53dQ11YiKX3");
pub const USDC_TOKEN_MINT_PUBKEY: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const CCTP_REMOTE_DOMAIN_ETH: u32 = 0u32;
pub const CCTP_MESSAGE_TRANSMITTER: Pubkey =
    pubkey!("BWrwSWjbikT3H7qHAkUEbLmwDQoB4ZDJ4wcSEhSPTZCu");
pub const CCTP_TOKEN_MESSENGER: Pubkey = pubkey!("Afgq3BHEfCE7d78D2XE9Bfyu2ieDqvE24xX8KDwreBms");
pub const CCTP_TOKEN_MINTER: Pubkey = pubkey!("DBD8hAwLDRQkTsu6EqviaYNGKPnsAMmQonxf7AH8ZcFY");
pub const CCTP_LOCAL_TOKEN: Pubkey = pubkey!("72bvEFk2Usi2uYc1SnaTNhBcQPc6tiJWXr9oKk7rkd4C");
pub const CCTP_REMOTE_TOKEN_MESSENGER: Pubkey =
    pubkey!("Hazwi3jFQtLKc2ughi7HFXPkpDeso7DQaMR9Ks4afh3j");

// Layer Zero OFT
pub const USDS_TOKEN_MINT_PUBKEY: Pubkey = pubkey!("AtGakZsHVY1BkinHEFMEJxZYhwA9KnuLD8QRmGjSAZEC");
pub const LZ_USDS_PEER_CONFIG_PUBKEY: Pubkey =
    pubkey!("EZ4hoYu18tVZBYjw7rdVGahHbyuwakukw2zHNvvMHyjR");
pub const LZ_USDS_OFT_STORE_PUBKEY: Pubkey =
    pubkey!("HUPW9dJZxxSafEVovebGxgbac3JamjMHXiThBxY5u43M");
pub const LZ_DESTINATION_DOMAIN_EID: u32 = 40106u32;
pub const LZ_USDS_ESCROW: Pubkey = pubkey!("HwpzV5qt9QzYRuWkHqTRuhbqtaMhapSNuriS5oMynkny");
pub const LZ_MSG_LIB: Pubkey = pubkey!("2XgGZG4oP29U3w5h4nTk1V2LFHL23zKDPJjs3psGzLKQ");

// LZ Required Programs
pub const LZ_USDS_OFT_PROGRAM_ID: Pubkey = pubkey!("E2R6qMMzLBjCwXs66MPEg2zKfpt5AMxWNgSULsLYfPS2");
pub const LZ_ENDPOINT_PROGRAM_ID: Pubkey = pubkey!("76y77prsiCMvXMjuoZ5VRrhG5qYBrUMYTE5WgHqgjEn6");
pub const LZ_ULN302: Pubkey = pubkey!("7a4WjyR8VZ7yZz5XJAKm39BUGn5iT9CKcv2pmG9tdXVH");
pub const LZ_EXECUTOR_PROGRAM_ID: Pubkey = pubkey!("6doghB248px58JSSwG4qejQ46kFMW4AMj7vzJnWZHNZn");
pub const LZ_R1_PROGRAM_ID: Pubkey = pubkey!("8ahPGPjEbpgGaZx2NV1iG5Shj7TDwvsjkEDcGWjt94TP");
pub const LZ_R2_PROGRAM_ID: Pubkey = pubkey!("HtEYV4xB4wvsj5fgTkcfuChYpvGYzgzwvNhgDZQNh7wW");

// TODO fix
pub const DEVNET_RPC: &str = "https://api.devnet.solana.com";


// Kamino Lend
pub const KAMINO_LEND_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const KAMINO_FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
pub const KAMINO_MAIN_MARKET: Pubkey = pubkey!("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF");
pub const KAMINO_USDC_RESERVE: Pubkey = pubkey!("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59");
pub const KAMINO_USDC_RESERVE_FARM_COLLATERAL: Pubkey = pubkey!("JAvnB9AKtgPsTEoKmn24Bq64UMoYcrtWtq42HHBdsPkh");
pub const KAMINO_REFERRER_METADATA: Pubkey = pubkey!("Bp5TLBJ53fcGMCnxQg8FSwHg9xDu2CuMffvT5uxkRcNA");
pub const LUT_PROGRAM_ID: Pubkey = pubkey!("AddressLookupTab1e1111111111111111111111111");
pub const KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY: Pubkey = pubkey!("Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6");
pub const KAMINO_USDC_RESERVE_COLLATERAL_MINT: Pubkey = pubkey!("B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D");
pub const KAMINO_USDC_RESERVE_COLLATERAL_SUPPLY: Pubkey = pubkey!("3DzjXRfxRm6iejfyyMynR4tScddaanrePJ1NJU2XnPPL");
pub const KAMINO_USDC_RESERVE_SCOPE_CONFIG_PRICE_FEED: Pubkey= pubkey!("3NJYftD5sjVfxSnUdZ1wVML8f3aC6mp1CXCL6L7TnU8C");
pub const KAMINO_USDC_RESERVE_FARM_GLOBAL_CONFIG: Pubkey = pubkey!("6UodrBjL2ZreDy7QdR4YV1oxqMBjVYSEyrFpctqqwGwL");
pub const BONK_MINT: Pubkey = pubkey!("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263");
pub const KAMINO_USDC_RESERVE_BONK_VAULT: Pubkey = pubkey!("5JehzcYZjvqhijhhavULvgsM9BQfRyqVoztqhfJ7mdBn");
pub const KAMINO_USDC_RESERVE_BONK_TREASURY_VAULT: Pubkey = pubkey!("76Nzx5DqDre5SSP7wyNTHzYTvqeQaf1V9ZSCV7pofcH8");
pub const KAMINO_RESERVE_FARM_DEBT: Pubkey = pubkey!("87gUNr8LwYJCT25HjPEHnrfBBjwEMAjfqCfnKcJNqy9Y");
pub const KAMINO_OTHER_MARKET: Pubkey = pubkey!("6WEGfej9B9wjxRs6t4BYpb9iCXd8CpTpJ8fVSNzHCC5y");
pub const KAMINO_OTHER_MARKET_RESERVE: Pubkey = pubkey!("Atj6UREVWa7WxbF2EMKNyfmYUY1U1txughe2gjhcPDCo");
pub const KAMINO_OTHER_MARKET_FARM_COLLATERAL: Pubkey = pubkey!("6Y9fzrWzGZaxdAJ2eWRg9UZpL3kqPDiVXAb67KJpWdUg");
pub const KAMINO_OTHER_MARKET_FARM_DEBT: Pubkey = pubkey!("87gUNr8LwYJCT25HjPEHnrfBBjwEMAjfqCfnKcJNqy9Y");
pub const KAMINO_OTHER_MARKET_RESERVE_LIQ_SUPPLY: Pubkey = pubkey!("BBcwMNSMyhhBnYE9pevEvkxKHGzTafMP9v3j7Kk7nAWM");
pub const KAMINO_OTHER_MARKET_RESERVE_COLLATERAL_MINT: Pubkey = pubkey!("6M89FWrQaqcy3domy85J1a1wVMnviL86WeUqbqTXf1qb");
pub const KAMINO_OTHER_MARKET_RESERVE_COLLATERAL_SUPPLY: Pubkey = pubkey!("25x4aEFoJE3bk4sdNLgHrrmchyop1JvcmGA4ccA6tWWT");
pub const KAMINO_OTHER_MARKET_REWARD_VAULT: Pubkey = pubkey!("8RykpmzzM5W5oRGwf8xqjpscpBtLvAFjfPBmkvfPUybD"); 
pub const KAMINO_OTHER_MARKET_TREASURY_VAULT: Pubkey = pubkey!("93Lk1EZAwofsGKeXcjxdVkkZi5mJTXSDaC3tdGz8tRBg"); 
pub const SYRUP_USDC_MINT: Pubkey = pubkey!("AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj");