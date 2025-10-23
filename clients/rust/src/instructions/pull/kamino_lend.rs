use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

use crate::{
    constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID},
    generated::{
        instructions::PullBuilder,
        types::{KaminoConfig, PullArgs},
    },
    integrations::kamino::{
        derive_market_authority_address, derive_obligation_farm_address,
        derive_reserve_collateral_mint, derive_reserve_collateral_supply,
        derive_reserve_liquidity_supply,
    },
    pda::{derive_controller_authority_pda, derive_permission_pda, derive_reserve_pda},
    SPL_TOKEN_PROGRAM_ID, SVM_ALM_CONTROLLER_ID,
};

/// Creates a `Pull` instruction to withdraw funds from a **Kamino Lend integration** under
/// the SVM ALM Controller program.
///
/// This instruction pulls liquidity from the Kamino lending market into the controller’s vault.
/// It sets up all the necessary PDAs and account references (reserves, obligations, collateral,
/// markets, farms, etc.) required to execute a withdrawal against the Kamino protocol.
///
/// # Parameters
///
/// - `controller`: The controller account that owns the integration.
/// - `integration`: The integration PDA for this Kamino Lend integration.
/// - `authority`: The authority allowed to perform the pull.
/// - `kamino_config`: Configuration object describing the Kamino market, reserve, and farm accounts.
/// - `amount`: The amount of collateral to pull.
///
/// # Derived Accounts
///
/// Internally derives:
/// - **Permission PDA**.
/// - **Controller Authority PDA**.
/// - **Vault ATA**.
/// - **Reserve PDA**.
/// - **Obligation Farm Collateral PDA**.
/// - **Market Authority PDA**.
/// - **Kamino Reserve PDAs**.
///
/// # Returns
///
/// - `Instruction`: The fully built Solana instruction ready to be sent.
///
pub fn create_pull_kamino_lend_ix(
    controller: &Pubkey,
    integration: &Pubkey,
    authority: &Pubkey,
    kamino_config: &KaminoConfig,
    reserve_farm_collateral: &Pubkey,
    liquidity_token_program: &Pubkey,
    amount: u64,
) -> Instruction {
    let calling_permission_pda = derive_permission_pda(controller, authority);
    let controller_authority = derive_controller_authority_pda(controller);
    let obligation = kamino_config.obligation;
    let kamino_reserve = kamino_config.reserve;
    let kamino_market = kamino_config.market;
    let kamino_reserve_liquidity_mint = kamino_config.reserve_liquidity_mint;
    let kamino_reserve_liquidity_supply =
        derive_reserve_liquidity_supply(&kamino_market, &kamino_reserve_liquidity_mint);
    let kamino_reserve_collateral_mint =
        derive_reserve_collateral_mint(&kamino_market, &kamino_reserve_liquidity_mint);
    let kamino_reserve_collateral_supply =
        derive_reserve_collateral_supply(&kamino_market, &kamino_reserve_liquidity_mint);
    let (market_authority, _) = derive_market_authority_address(&kamino_market);
    // if reserve_farm_collateral is not set, we need to pass KAMINO_LEND_PROGRAM_ID (None in the Optional account)
    let obligation_farm_collateral = if reserve_farm_collateral == &Pubkey::default() {
        KAMINO_LEND_PROGRAM_ID
    } else {
        derive_obligation_farm_address(reserve_farm_collateral, &obligation)
    };

    let reserve_pda = derive_reserve_pda(controller, &kamino_reserve_liquidity_mint);
    let vault = get_associated_token_address_with_program_id(
        &controller_authority,
        &kamino_reserve_liquidity_mint,
        liquidity_token_program,
    );

    let remaining_accounts = &[
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: obligation,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: kamino_reserve,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: kamino_reserve_liquidity_mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: kamino_reserve_liquidity_supply,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: kamino_reserve_collateral_mint,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: kamino_reserve_collateral_supply,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: market_authority,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: kamino_market,
            is_signer: false,
            is_writable: false,
        },
        // collateral token program
        AccountMeta {
            pubkey: SPL_TOKEN_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        // liquidity token program
        AccountMeta {
            pubkey: *liquidity_token_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: sysvar::instructions::ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: obligation_farm_collateral,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: *reserve_farm_collateral,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: KAMINO_FARMS_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: KAMINO_LEND_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
    ];

    PullBuilder::new()
        .pull_args(PullArgs::Kamino { amount })
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .integration(*integration)
        .reserve_a(reserve_pda)
        .program_id(SVM_ALM_CONTROLLER_ID)
        .add_remaining_accounts(remaining_accounts)
        .instruction()
}
