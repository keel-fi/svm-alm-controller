use solana_pubkey::Pubkey;

/// Derive PDA for Integration account
pub fn derive_integration_pda(controller_pda: &Pubkey, hash: &[u8; 32]) -> Pubkey {
    let (integration_pda, _integration_bump) = Pubkey::find_program_address(
        &[b"integration", &controller_pda.to_bytes(), &hash.as_ref()],
        &crate::SVM_ALM_CONTROLLER_ID,
    );
    integration_pda
}

/// Derive PDA for Permission account address
pub fn derive_permission_pda(controller_pda: &Pubkey, authority: &Pubkey) -> Pubkey {
    let (permission_pda, _permission_bump) = Pubkey::find_program_address(
        &[
            b"permission",
            &controller_pda.to_bytes(),
            &authority.to_bytes(),
        ],
        &crate::SVM_ALM_CONTROLLER_ID,
    );
    permission_pda
}

/// Derive Controller account address
pub fn derive_controller_pda(id: &u16) -> Pubkey {
    let (controller_pda, _controller_bump) = Pubkey::find_program_address(
        &[b"controller", &id.to_le_bytes()],
        &crate::SVM_ALM_CONTROLLER_ID,
    );
    controller_pda
}

/// Derive Controller authority address
pub fn derive_controller_authority_pda(controller_pda: &Pubkey) -> Pubkey {
    let (controller_authority_pda, _controller_authority_bump) = Pubkey::find_program_address(
        &[b"controller_authority", controller_pda.as_ref()],
        &crate::SVM_ALM_CONTROLLER_ID,
    );
    controller_authority_pda
}

pub fn derive_reserve_pda(controller_pda: &Pubkey, mint: &Pubkey) -> Pubkey {
    let (reserve_pda, _reserve_bump) = Pubkey::find_program_address(
        &[b"reserve", &controller_pda.to_bytes(), &mint.to_bytes()],
        &crate::SVM_ALM_CONTROLLER_ID,
    );
    reserve_pda
}

pub fn derive_oracle_pda(nonce: &Pubkey) -> Pubkey {
    let (oracle_pda, _bump) = Pubkey::find_program_address(
        &[b"oracle", &nonce.to_bytes()],
        &crate::SVM_ALM_CONTROLLER_ID,
    );
    oracle_pda
}
