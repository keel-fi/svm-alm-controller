use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;

use crate::{
    constants::{ASSOCIATED_TOKEN_PROGRAM_ID, KAMINO_FARMS_PROGRAM_ID},
    generated::{instructions::SyncBuilder, types::KaminoConfig},
    integrations::kamino::{
        derive_farm_vaults_authority, derive_obligation_farm_address,
        derive_rewards_treasury_vault, derive_rewards_vault,
    },
    pda::{derive_controller_authority_pda, derive_reserve_pda},
};

pub struct HarvestRewardAccounts<'a> {
    pub rewards_mint: &'a Pubkey,
    pub global_config: &'a Pubkey,
    pub reserve_farm_collateral: &'a Pubkey,
    pub scope_prices: &'a Pubkey,
    pub rewards_token_program: &'a Pubkey,
}

/// Creates a `Sync` instruction for a **Kamino Lend integration** under the
/// SVM ALM Controller program.
///
/// This instruction synchronizes the controller’s accounting with Kamino and harvests rewards (optional).
///
/// # Parameters
///
/// - `controller`: The controller account that owns the integration.
/// - `integration`: The integration PDA for this Kamino Lend integration
/// - `authority`: The authority allowed to perform the pull.
/// - `liquidity_token_program`: the token program of the integration and kamino_reserve mint.
/// - `harvest_rewards_accounts`: Optional accounts used for harvesting rewards.
///
/// # Derived Accounts
///
/// Internally derives:
/// - **Controller Authority PDA**
/// - **Vault ATA**
/// - **Reserve PDA**
/// - **Obligation Farm PDA**
/// - **Rewards Vault PDA**
/// - **Rewards Treasury Vault PDA**
/// - **Farms Vault Authority PDA**
/// - **Rewards ATA**
///
/// # Returns
///
/// - `Instruction`: A fully constructed Solana instruction ready to submit.
///
pub fn create_sync_kamino_lend_ix(
    controller: &Pubkey,
    integration: &Pubkey,
    payer: &Pubkey,
    kamino_config: &KaminoConfig,
    liquidity_token_program: &Pubkey,
    harvest_rewards_accounts: Option<HarvestRewardAccounts>,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);

    let vault = get_associated_token_address_with_program_id(
        &controller_authority,
        &kamino_config.reserve_liquidity_mint,
        &liquidity_token_program,
    );

    let obligation = kamino_config.obligation;
    let kamino_reserve = kamino_config.reserve;
    let kamino_reserve_liquidity_mint = kamino_config.reserve_liquidity_mint;
    let reserve_pda = derive_reserve_pda(controller, &kamino_reserve_liquidity_mint);

    let mut remaining_accounts = vec![
        AccountMeta {
            pubkey: vault,
            is_writable: true,
            is_signer: false,
        },
        AccountMeta {
            pubkey: kamino_reserve,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: obligation,
            is_signer: false,
            is_writable: false,
        },
    ];

    if let Some(HarvestRewardAccounts {
        rewards_mint,
        global_config,
        reserve_farm_collateral,
        scope_prices,
        rewards_token_program,
    }) = harvest_rewards_accounts
    {
        let reserve_farm = reserve_farm_collateral;
        let obligation_farm_pda = derive_obligation_farm_address(reserve_farm, &obligation);
        let rewards_vault_pda = derive_rewards_vault(reserve_farm, &rewards_mint);
        let rewards_treasury_vault_pda =
            derive_rewards_treasury_vault(&global_config, &rewards_mint);
        let (farms_vault_authority_pda, _) = derive_farm_vaults_authority(reserve_farm);
        let rewards_ata = get_associated_token_address_with_program_id(
            &controller_authority,
            &rewards_mint,
            rewards_token_program,
        );
        remaining_accounts.extend(vec![
            AccountMeta {
                pubkey: obligation_farm_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *reserve_farm,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: rewards_vault_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: rewards_treasury_vault_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: farms_vault_authority_pda,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: *global_config,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: rewards_ata,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *rewards_mint,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: *scope_prices,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: *rewards_token_program,
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
                pubkey: ASSOCIATED_TOKEN_PROGRAM_ID,
                is_signer: false,
                is_writable: false,
            },
        ]);
    }

    SyncBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .payer(*payer)
        .integration(*integration)
        .reserve(reserve_pda)
        .add_remaining_accounts(&remaining_accounts)
        .instruction()
}
