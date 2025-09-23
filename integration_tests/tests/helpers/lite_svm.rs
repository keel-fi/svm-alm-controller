use base64;
use serde_json::Value;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use std::{fs, str::FromStr};

/// Load an account in compact-json format from a json file.
pub fn get_account_data_from_json(path: &str) -> Account {
    let json_data = fs::read_to_string(path).expect("Unable to read JSON file");
    let v: Value = serde_json::from_str(&json_data).expect("Unable to parse JSON");

    let lamports = v["account"]["lamports"]
        .as_u64()
        .expect("Expected lamports as u64");
    let base64_data = v["account"]["data"][0].as_str().expect("Expected a string");
    let data = base64::decode(base64_data).expect("Failed to decode base64");
    let owner_str = v["account"]["owner"]
        .as_str()
        .expect("Expected owner as string");
    let owner = Pubkey::from_str(owner_str).expect("Invalid owner pubkey");
    let executable = v["account"]["executable"]
        .as_bool()
        .expect("Expected executable as bool");
    let rent_epoch = v["account"]["rentEpoch"]
        .as_u64()
        .expect("Expected rentEpoch as u64");

    Account {
        lamports,
        data,
        owner,
        executable,
        rent_epoch,
    }
}
