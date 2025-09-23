use hex;
use solana_sdk::pubkey::Pubkey;
use std::string::String;

fn derive_token_messenger_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"token_messenger"], program_id);
    pda
}

fn derive_message_transmitter_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"message_transmitter"], program_id);
    pda
}

fn derive_token_minter_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"token_minter"], program_id);
    pda
}

fn derive_local_token_pda(mint_pubkey: Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) =
        Pubkey::find_program_address(&[b"local_token", mint_pubkey.as_ref()], program_id);
    pda
}

fn derive_remote_token_messenger_pda(remote_domain: &str, program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(
        &[b"remote_token_messenger", remote_domain.as_ref()],
        program_id,
    );
    pda
}

fn derive_sender_authority_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"sender_authority"], program_id);
    pda
}
fn derive_event_authority_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"__event_authority"], program_id);
    pda
}

// Struct to hold the programs
#[derive(Debug)]
pub struct CctpDepositForBurnPdas {
    pub message_transmitter: Pubkey,
    pub token_messenger: Pubkey,
    pub token_minter: Pubkey,
    pub local_token: Pubkey,
    pub remote_token_messenger: Pubkey,
    pub sender_authority: Pubkey,
    pub event_authority: Pubkey,
}

impl CctpDepositForBurnPdas {
    pub fn derive(
        message_transmitter_program_id: Pubkey,
        token_messenger_minter_program_id: Pubkey,
        mint_pubkey: Pubkey,
        remote_domain: u32,
    ) -> Self {
        let message_transmitter = derive_message_transmitter_pda(&message_transmitter_program_id);
        let token_messenger = derive_token_messenger_pda(&token_messenger_minter_program_id);
        let token_minter = derive_token_minter_pda(&token_messenger_minter_program_id);
        let local_token = derive_local_token_pda(mint_pubkey, &token_messenger_minter_program_id);
        let remote_token_messenger = derive_remote_token_messenger_pda(
            &remote_domain.to_string(),
            &token_messenger_minter_program_id,
        );
        let sender_authority = derive_sender_authority_pda(&token_messenger_minter_program_id);
        let event_authority = derive_event_authority_pda(&token_messenger_minter_program_id);
        Self {
            message_transmitter,
            token_messenger,
            token_minter,
            local_token,
            remote_token_messenger,
            sender_authority,
            event_authority,
        }
    }
}

/// Converts an Ethereum address from hexadecimal to a base58-encoded string.
pub fn evm_address_to_solana_pubkey(evm_address: &str) -> Pubkey {
    let addr = hex::decode(evm_address.trim_start_matches("0x")).expect("Invalid hex string");
    assert_eq!(addr.len(), 20, "Expected 20-byte Ethereum address");
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(&addr); // left-pad with zeros
    Pubkey::new_from_array(bytes)
}

/// Converts an Ethereum address to a 32-byte hexadecimal string.
fn evm_address_to_bytes32(address: &str) -> String {
    format!("0x{:0>64}", address.trim_start_matches("0x"))
}

/// Converts a hexadecimal string to a byte array.
fn hex_to_bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex.trim_start_matches("0x")).expect("Invalid hex string")
}
