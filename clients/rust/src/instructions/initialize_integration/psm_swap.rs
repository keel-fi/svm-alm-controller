use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_program::{keccak::hash, system_program};
use crate::{derive_controller_authority_pda, derive_integration_pda, derive_permission_pda, generated::{instructions::InitializeIntegrationBuilder, types::{
    InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType, PsmSwapConfig
}}};

pub fn create_psm_swap_initialize_integration_instruction(
    payer: &Pubkey,
    controller: &Pubkey,
    authority: &Pubkey,
    mint: &Pubkey,
    description: &str,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    permit_liquidation: bool,
    psm_pool: &Pubkey,
    psm_token: &Pubkey,
    psm_token_vault: &Pubkey,
) -> (Instruction, Pubkey) {
    let config = IntegrationConfig::PsmSwap(PsmSwapConfig {
        psm_token: *psm_token,
        psm_pool: *psm_pool,
        mint: *mint,
        padding: [0u8; 128],
    });

    let inner_args = InitializeArgs::PsmSwap;

    let hash = hash(borsh::to_vec(&config).unwrap().as_ref()).to_bytes();
    let integration_pda = derive_integration_pda(controller, &hash);
    let permission_pda = derive_permission_pda(controller, authority);
    let controller_authority = derive_controller_authority_pda(controller);

    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

    let remaining_accounts = [
        AccountMeta {
            pubkey: *psm_pool,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *psm_token,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *psm_token_vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *mint,
            is_signer: false,
            is_writable: true,
        },
    ];

    let ix = InitializeIntegrationBuilder::new()
        .integration_type(IntegrationType::PsmSwap)
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
        .instruction();

    (ix, integration_pda)
}
