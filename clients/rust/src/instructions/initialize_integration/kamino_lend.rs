use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    keccak::hash,
    pubkey::Pubkey,
    system_program,
    sysvar::rent,
};

use crate::{
    constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID},
    derive_controller_authority_pda, derive_integration_pda, derive_permission_pda,
    generated::{
        instructions::InitializeIntegrationBuilder,
        types::{InitializeArgs, IntegrationConfig, IntegrationStatus, IntegrationType},
    },
    integrations::kamino::{
        derive_market_authority_address, derive_obligation_farm_address,
        derive_user_metadata_address,
    },
};

/// Creates an `InitializeIntegration` instruction for a **Kamino Lend integration** under the
/// SVM ALM Controller program.
///
/// This instruction sets up a new integration between the controller and the Kamino Lend protocol,
/// registering the configuration, rate limits, and metadata. It prepares all the required PDAs and
/// account references (obligations, reserves, farms, lookup tables, etc.) and returns both the
/// constructed `Instruction` and the derived integration PDA.
///
/// # Parameters
///
/// - `controller`: The controller account that will own the integration.
/// - `payer`: The account funding the initialization transaction.
/// - `authority`: The authority allowed to manage this integration.
/// - `description`: A string description of the integration (truncated/padded to 32 bytes).
/// - `status`: Initial status of the integration (e.g. Active/Suspended).
/// - `rate_limit_slope`: The rate limit slope parameter for the integration.
/// - `rate_limit_max_outflow`: The maximum rate limit outflow for the integration.
/// - `config`: Integration configuration. Must be of type `IntegrationConfig::UtilizationMarket::Kamino`.
/// - `slot`: The current slot, used for deriving the lookup table PDA.
/// - `obligation_id`: An identifier for the Kamino obligation.
/// - `referrer`: Pubkey of a referrer (optional, has to set to KLEND_PROGRAM_ID for None)
///
/// # Derived Accounts
///
/// Internally derives:
/// - **Integration PDA**.
/// - **Permission PDA**.
/// - **Controller Authority PDA**.
/// - **User Metadata PDA**.
/// - **User Lookup Table PDA**.
/// - **Obligation Farm PDAs**.
/// - **Market Authority PDA**.
///
/// # Returns
///
/// - `(Instruction, Pubkey)` tuple where:
///   - `Instruction`: The fully built Solana instruction ready to be sent.
///   - `Pubkey`: The integration PDA associated with this Kamino Lend integration.
///
/// # Panics
///
/// This function will panic if `config` is not of type
/// `IntegrationConfig::UtilizationMarket::Kamino`.
pub fn create_initialize_kamino_lend_integration_ix(
    controller: &Pubkey,
    payer: &Pubkey,
    authority: &Pubkey,
    description: &str,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    permit_liquidation: bool,
    config: &IntegrationConfig,
    reserve_farm_collateral: &Pubkey,
    obligation_id: u8,
    referrer: &Pubkey,
) -> (Instruction, Pubkey) {
    let calling_permission_pda = derive_permission_pda(controller, authority);
    let controller_authority = derive_controller_authority_pda(controller);
    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);
    let hash = hash(borsh::to_vec(config).unwrap().as_ref()).to_bytes();
    let integration_pda = derive_integration_pda(controller, &hash);

    let kamino_config = match config {
        IntegrationConfig::Kamino(kamino_config) => kamino_config,
        _ => panic!("config error"),
    };

    let obligation = kamino_config.obligation;
    let market = kamino_config.market;
    let reserve_liquidity_mint = kamino_config.reserve_liquidity_mint;
    let reserve = kamino_config.reserve;
    let user_metadata = derive_user_metadata_address(&controller_authority);
    let obligation_farm_collateral =
        derive_obligation_farm_address(&reserve_farm_collateral, &obligation);
    let (market_authority, _) = derive_market_authority_address(&market);

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
            pubkey: *referrer,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: obligation_farm_collateral,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: reserve,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *reserve_farm_collateral,
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
        .integration_type(IntegrationType::Kamino)
        .status(status)
        .description(description_encoding)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .permit_liquidation(permit_liquidation)
        .inner_args(InitializeArgs::KaminoIntegration { obligation_id })
        .payer(*payer)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .integration(integration_pda)
        .add_remaining_accounts(remaining_accounts)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .system_program(system_program::ID)
        .instruction();

    (instruction, integration_pda)
}
