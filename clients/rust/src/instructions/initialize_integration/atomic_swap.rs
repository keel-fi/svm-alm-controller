use solana_instruction::{AccountMeta, Instruction};
use solana_program::{keccak::hash, system_program};
use solana_pubkey::Pubkey;

use crate::{
    derive_controller_authority_pda, derive_integration_pda, derive_permission_pda,
    generated::{
        instructions::InitializeIntegrationBuilder,
        types::{
            AtomicSwapConfig, InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType,
        },
    },
};

/// Instruction generation for initializing AtomicSwap integration
pub fn create_atomic_swap_initialize_integration_instruction(
    payer: &Pubkey,
    controller: &Pubkey,
    authority: &Pubkey,
    description: &str,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    permit_liquidation: bool,
    input_token: &Pubkey,
    input_mint_decimals: u8,
    output_token: &Pubkey,
    output_mint_decimals: u8,
    oracle: &Pubkey,
    max_staleness: u64,
    expiry_timestamp: i64,
    max_slippage_bps: u16,
    oracle_price_inverted: bool,
) -> Instruction {
    let config = IntegrationConfig::AtomicSwap(AtomicSwapConfig {
        input_token: *input_token,
        output_token: *output_token,
        oracle: *oracle,
        max_staleness,
        expiry_timestamp,
        max_slippage_bps,
        input_mint_decimals,
        output_mint_decimals,
        oracle_price_inverted,
        padding: [0u8; 107],
    });

    let inner_args = InitializeArgs::AtomicSwap {
        max_slippage_bps,
        max_staleness,
        expiry_timestamp,
        oracle_price_inverted,
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
            pubkey: *input_token,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *output_token,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *oracle,
            is_signer: false,
            is_writable: false,
        },
    ];

    InitializeIntegrationBuilder::new()
        .integration_type(IntegrationType::AtomicSwap)
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
