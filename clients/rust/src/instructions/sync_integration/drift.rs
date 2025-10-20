use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{
    derive_controller_authority_pda,
    generated::instructions::SyncBuilder,
    integrations::drift::{
        derive_spot_market_pda, derive_spot_market_vault_pda, derive_user_pda, DRIFT_PROGRAM_ID,
    },
};

pub fn create_drift_sync_integration_instruction(
    controller: &Pubkey,
    payer: &Pubkey,
    integration: &Pubkey,
    reserve: &Pubkey,
    spot_market_index: u16,
    sub_account_id: u16,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let controller_authority = derive_controller_authority_pda(controller);

    // Derive the required drift PDAs
    let spot_market_vault = derive_spot_market_vault_pda(spot_market_index);
    let spot_market = derive_spot_market_pda(spot_market_index);
    let user = derive_user_pda(&controller_authority, sub_account_id);

    let remaining_accounts = &[
        AccountMeta {
            pubkey: spot_market_vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: spot_market,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: user,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: DRIFT_PROGRAM_ID,
            is_signer: false,
            is_writable: false,
        },
    ];

    let instruction = SyncBuilder::new()
        .controller(*controller)
        .controller_authority(controller_authority)
        .payer(*payer)
        .integration(*integration)
        .reserve(*reserve)
        .add_remaining_accounts(remaining_accounts)
        .instruction();
    Ok(instruction)
}
