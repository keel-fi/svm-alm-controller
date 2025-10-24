use solana_instruction::{AccountMeta, Instruction};
use solana_program::{keccak::hash, system_program};
use solana_pubkey::Pubkey;
use solana_sysvar::rent::ID as RENT_ID;

use crate::{
    derive_controller_authority_pda, derive_integration_pda, derive_permission_pda,
    generated::{
        instructions::InitializeIntegrationBuilder,
        types::{
            DriftConfig, InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType,
        },
    },
    integrations::drift::{
        derive_spot_market_pda, derive_state_pda, derive_user_pda, derive_user_stats_pda,
        DRIFT_PROGRAM_ID,
    },
};

/// Instruction generation for initializing Drift integration
pub fn create_drift_initialize_integration_instruction(
    payer: &Pubkey,
    controller: &Pubkey,
    authority: &Pubkey,
    mint: &Pubkey,
    description: &str,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    permit_liquidation: bool,
    sub_account_id: u16,
    spot_market_index: u16,
) -> Instruction {
    let config = IntegrationConfig::Drift(DriftConfig {
        sub_account_id,
        spot_market_index,
        padding: [0u8; 220],
    });

    let inner_args = InitializeArgs::Drift {
        sub_account_id,
        spot_market_index,
    };

    let hash = hash(borsh::to_vec(&config).unwrap().as_ref()).to_bytes();
    let integration_pda = derive_integration_pda(controller, &hash);
    let permission_pda = derive_permission_pda(controller, authority);
    let controller_authority = derive_controller_authority_pda(controller);

    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

    let user_stats = derive_user_stats_pda(&controller_authority);
    let user = derive_user_pda(&controller_authority, sub_account_id);
    let state = derive_state_pda();
    let spot_market = derive_spot_market_pda(spot_market_index);

    let remaining_accounts = [
        AccountMeta {
            pubkey: *mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: user,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: user_stats,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: state,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: spot_market,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: RENT_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: DRIFT_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
    ];

    InitializeIntegrationBuilder::new()
        .integration_type(IntegrationType::Drift)
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
