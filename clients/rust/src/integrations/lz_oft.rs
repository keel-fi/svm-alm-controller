use solana_pubkey::Pubkey;

pub fn derive_oft_store(token_escrow: &Pubkey, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"OFT", token_escrow.as_ref()], program_id).0
}

pub fn derive_peer_config(oft_store: &Pubkey, remote_eid: u32, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"Peer", oft_store.as_ref(), &remote_eid.to_be_bytes()],
        program_id,
    )
    .0
}
