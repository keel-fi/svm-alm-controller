use solana_pubkey::{pubkey, Pubkey};

// CCTP constants
pub const CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID: Pubkey =
    pubkey!("CCTPmbSD7gX1bxKPAmg77w8oFzNFpaQiQUWD43TKaecd");
pub const CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID: Pubkey =
    pubkey!("CCTPiPYPc6AsJuwueEnWgSgucamXDZwBd53dQ11YiKX3");

pub fn derive_token_messenger_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"token_messenger"], program_id);
    pda
}

pub fn derive_message_transmitter_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"message_transmitter"], program_id);
    pda
}

pub fn derive_token_minter_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"token_minter"], program_id);
    pda
}

pub fn derive_local_token_pda(mint_pubkey: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) =
        Pubkey::find_program_address(&[b"local_token", mint_pubkey.as_ref()], program_id);
    pda
}

pub fn derive_remote_token_messenger_pda(remote_domain: &str, program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(
        &[b"remote_token_messenger", remote_domain.as_ref()],
        program_id,
    );
    pda
}

pub fn derive_sender_authority_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"sender_authority"], program_id);
    pda
}
pub fn derive_event_authority_pda(program_id: &Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(&[b"__event_authority"], program_id);
    pda
}
