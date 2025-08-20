use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    keccak::hash,
    pubkey::Pubkey,
    system_program,
    sysvar::rent,
};

use crate::{
    constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, LUT_PROGRAM_ID},
    generated::{
        instructions::InitializeIntegrationBuilder,
        types::{
            InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType,
            UtilizationMarket, UtilizationMarketConfig,
        },
    },
    pdas::{
        derive_controller_authority_pda, derive_integration_pda, derive_lookup_table_address,
        derive_market_authority_address, derive_obligation_farm_address, derive_permission_pda,
        derive_user_metadata_address,
    },
    SVM_ALM_CONTROLLER_ID,
};

pub fn get_kamino_init_ix(
    controller: &Pubkey,
    payer: &Pubkey,
    authority: &Pubkey,
    description: &str,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    config: &IntegrationConfig,
    slot: u64,
    obligation_id: u8,
) -> (Instruction, Pubkey) {
    let calling_permission_pda = derive_permission_pda(controller, authority);
    let controller_authority = derive_controller_authority_pda(controller);
    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);
    let hash = hash(borsh::to_vec(config).unwrap().as_ref()).to_bytes();
    let integration_pda = derive_integration_pda(controller, &hash);

    let kamino_config = match config {
        IntegrationConfig::UtilizationMarket(c) => match c {
            UtilizationMarketConfig::KaminoConfig(kamino_config) => kamino_config,
        },
        _ => panic!("config error"),
    };

    let obligation = kamino_config.obligation;
    let market = kamino_config.market;
    let reserve_liquidity_mint = kamino_config.reserve_liquidity_mint;
    let reserve = kamino_config.reserve;
    let reserve_farm_collateral = kamino_config.reserve_farm_collateral;
    let reserve_farm_debt = kamino_config.reserve_farm_debt;
    let user_metadata = derive_user_metadata_address(&controller_authority);
    let user_lookup_table = derive_lookup_table_address(&controller_authority, slot);
    let obligation_farm_collateral =
        derive_obligation_farm_address(&reserve_farm_collateral, &obligation);
    let obligation_farm_debt = derive_obligation_farm_address(&reserve_farm_debt, &obligation);
    let market_authority = derive_market_authority_address(&market);

    let remaining_accounts = &[
        AccountMeta {
            pubkey: obligation,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: reserve_liquidity_mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: user_metadata,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: user_lookup_table,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: KAMINO_LEND_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: obligation_farm_collateral,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: obligation_farm_debt,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: reserve,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: reserve_farm_collateral,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: reserve_farm_debt,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: market_authority,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: market,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: LUT_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: KAMINO_LEND_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: KAMINO_FARMS_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: system_program::ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: rent::ID,
            is_signer: false,
            is_writable: false,
        },
    ];

    let instruction = InitializeIntegrationBuilder::new()
        .integration_type(IntegrationType::UtilizationMarket(
            UtilizationMarket::Kamino,
        ))
        .status(status)
        .description(description_encoding)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .inner_args(InitializeArgs::KaminoIntegration { obligation_id })
        .payer(*payer)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .integration(integration_pda)
        .lookup_table(system_program::ID)
        .add_remaining_accounts(remaining_accounts)
        .program_id(SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction();

    (instruction, integration_pda)
}
