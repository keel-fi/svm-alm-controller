use crate::subs::{
    derive_controller_authority_pda, derive_permission_pda,
};
use borsh::BorshDeserialize;
use litesvm::{types::TransactionResult, LiteSVM};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account_client::address::get_associated_token_address_with_program_id;
use svm_alm_controller::integrations::utilization_market::kamino::kamino_state::{KaminoReserve, Obligation};
use std::error::Error;
use svm_alm_controller_client::generated::{
    accounts::Integration, instructions::ManageIntegrationBuilder, programs::SVM_ALM_CONTROLLER_ID,
    types::IntegrationStatus,
};

pub fn derive_integration_pda(controller_pda: &Pubkey, hash: &[u8; 32]) -> Pubkey {
    let (integration_pda, _integration_bump) = Pubkey::find_program_address(
        &[b"integration", &controller_pda.to_bytes(), &hash.as_ref()],
        &Pubkey::from(SVM_ALM_CONTROLLER_ID),
    );
    integration_pda
}

pub fn fetch_integration_account(
    svm: &LiteSVM,
    integration_pda: &Pubkey,
) -> Result<Option<Integration>, Box<dyn Error>> {
    let info = svm.get_account(integration_pda);
    match info {
        Some(info) => {
            if info.data.is_empty() {
                Ok(None)
            } else {
                Integration::try_from_slice(&info.data[1..])
                    .map(Some)
                    .map_err(Into::into)
            }
        }
        None => Ok(None),
    }
}

pub fn manage_integration(
    svm: &mut LiteSVM,
    controller: &Pubkey,
    integration: &Pubkey,
    authority: &Keypair,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
    description: String,
) -> TransactionResult  {
    let calling_permission_pda = derive_permission_pda(controller, &authority.pubkey());
    let controller_authority = derive_controller_authority_pda(controller);

    let description_bytes = description.as_bytes();
    let mut description_encoding: [u8; 32] = [0; 32];
    description_encoding[..description_bytes.len()].copy_from_slice(description_bytes);

    let ixn = ManageIntegrationBuilder::new()
        .status(status)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .description(description_encoding)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(authority.pubkey())
        .permission(calling_permission_pda)
        .integration(*integration)
        .program_id(svm_alm_controller_client::SVM_ALM_CONTROLLER_ID)
        .instruction();

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(txn)
}
