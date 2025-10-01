use solana_instruction::{AccountMeta, Instruction};
use solana_program::{keccak::hash, system_program};
use solana_pubkey::Pubkey;

use crate::{
    derive_controller_authority_pda, derive_integration_pda, derive_permission_pda,
    generated::{
        instructions::InitializeIntegrationBuilder,
        types::{
            InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType, LzBridgeConfig,
        },
    },
    integrations::lz_oft,
};

/// Instruction generation for initializing LZ Bridge integration
pub fn create_lz_bridge_initialize_integration_instruction(
    payer: &Pubkey,
    controller: &Pubkey,
    authority: &Pubkey,
    description: &str,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    permit_liquidation: bool,
    oft_program_id: &Pubkey,
    oft_token_escrow: &Pubkey,
    destination_address: &Pubkey,
    destination_eid: u32,
    mint: &Pubkey,
) -> Instruction {
    let oft_store = lz_oft::derive_oft_store(oft_token_escrow, oft_program_id);
    let peer_config = lz_oft::derive_peer_config(&oft_store, destination_eid, oft_program_id);
    let config = IntegrationConfig::LzBridge(LzBridgeConfig {
        program: *oft_program_id,
        mint: *mint,
        oft_store,
        peer_config,
        oft_token_escrow: *oft_token_escrow,
        destination_address: *destination_address,
        destination_eid,
        padding: [0u8; 28],
    });

    let inner_args = InitializeArgs::LzBridge {
        destination_address: *destination_address,
        destination_eid,
    };

    let hash = hash(borsh::to_vec(&config).unwrap().as_ref()).to_bytes();
    let integration_pda = derive_integration_pda(controller, &hash);
    let permission_pda = derive_permission_pda(controller, authority);
    let controller_authority = derive_controller_authority_pda(controller);

    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

    let remaining_accounts = [
        AccountMeta {
            pubkey: *mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: oft_store,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: peer_config,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *oft_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *oft_token_escrow,
            is_signer: false,
            is_writable: false,
        },
    ];

    InitializeIntegrationBuilder::new()
        .integration_type(IntegrationType::LzBridge)
        .status(status)
        .description(description_encoding)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .permit_liquidation(permit_liquidation)
        .inner_args(inner_args.clone())
        .payer(*payer)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(permission_pda)
        .integration(integration_pda)
        .add_remaining_accounts(&remaining_accounts)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction()
}
