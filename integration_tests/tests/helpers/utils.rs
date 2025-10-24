use solana_sdk::pubkey::Pubkey;

/// Get the encoded Program return data from a list of log messages.
pub fn get_program_return_data(logs: Vec<String>, program_id: &Pubkey) -> Option<Vec<u8>> {
    let log_prefix = format!("Program return: {:?}", program_id);
    let log_prefix = log_prefix.as_str();
    for log in logs {
        if log.starts_with(log_prefix) {
            let encoded_data = log.trim_start_matches(log_prefix).trim();
            let decoded_data = base64::decode(encoded_data).unwrap();
            return Some(decoded_data);
        }
    }
    None
}
