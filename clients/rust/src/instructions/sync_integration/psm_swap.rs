use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{
    derive_controller_authority_pda, derive_reserve_pda,
    generated::{instructions::SyncBuilder, types::PsmSwapConfig},
};

/// Creates a `Sync` instruction for a **PSM Swap integration** under the
/// SVM ALM Controller program.
///
/// This instruction synchronizes the controller's accounting with the PSM Swap protocol.
///
/// # Parameters
///
/// - `controller`: The controller account that owns the integration.
/// - `payer`: The account that pays for the transaction fees.
/// - `integration`: The integration PDA for this PSM Swap integration.
/// - `psm_swap_config`: The PSM Swap configuration from the integration.
/// - `mint`: The mint of the token being swapped.
/// - `vault`: The vault account (should be the PSM token vault, which should match the reserve vault).
///
/// # Derived Accounts
///
/// Internally derives:
/// - **Controller Authority PDA**
/// - **Reserve PDA**
///
/// # Returns
///
/// - `Instruction`: A fully constructed Solana instruction ready to submit.
///
pub fn create_psm_swap_sync_integration_instruction(
    controller: &Pubkey,
    payer: &Pubkey,
    integration: &Pubkey,
    psm_swap_config: &PsmSwapConfig,
    mint: &Pubkey,
    vault: &Pubkey,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let controller_authority = derive_controller_authority_pda(controller);
    let reserve_pda = derive_reserve_pda(controller, mint);

    // For PSM swap, we need to read the PSM token account to get the vault
    // The vault passed should be the PSM token vault, not the reserve vault
    // Note: The client should read the PSM token account and pass its vault

    let remaining_accounts = &[
        AccountMeta {
            pubkey: *vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: psm_swap_config.psm_token,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: psm_swap_config.psm_pool,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: *mint,
            is_signer: false,
            is_writable: false,
        },
    ];

    let instruction = SyncBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .payer(*payer)
        .integration(*integration)
        .reserve(reserve_pda)
        .add_remaining_accounts(remaining_accounts)
        .instruction();
    Ok(instruction)
}
