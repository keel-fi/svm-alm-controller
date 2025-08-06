use std::str::FromStr;

use solana_sdk::{pubkey, pubkey::Pubkey};

// Nova Token Swap
pub const NOVA_TOKEN_SWAP_PROGRAM_ID: Pubkey =
    pubkey!("GnsdTwuo44397Bva92s9G4sdCU4Xkbf8ALeXcssxRxyi");
pub const NOVA_TOKEN_SWAP_FEE_OWNER: Pubkey =
    pubkey!("GnsdTwuo44397Bva92s9G4sdCU4Xkbf8ALeXcssxRxyi");

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

// Test program for TransferHook
pub const TEST_TRANSFER_HOOK_PROGRAM_ID: Pubkey = pubkey!("GrRNrGNoaRU47svzEseTSsD3dmPz9nCUVPDynFuW5WRm");

pub const DEVNET_RPC: &str = "https://api.devnet.solana.com";
