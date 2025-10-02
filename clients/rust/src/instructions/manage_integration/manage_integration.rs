use crate::{
    derive_controller_authority_pda, derive_permission_pda,
    generated::{instructions::ManageIntegrationBuilder, types::IntegrationStatus},
};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

/// Instruction generation for managing a integration account

pub fn create_manage_integration_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    integration: &Pubkey,
    status: IntegrationStatus,
    rate_limit_slope: u64,
    rate_limit_max_outflow: u64,
) -> Instruction {
    let calling_permission_pda = derive_permission_pda(controller, &authority);
    let controller_authority = derive_controller_authority_pda(controller);

    ManageIntegrationBuilder::new()
        .status(status)
        .rate_limit_slope(rate_limit_slope)
        .rate_limit_max_outflow(rate_limit_max_outflow)
        .controller(*controller)
        .controller_authority(controller_authority)
        .authority(*authority)
        .permission(calling_permission_pda)
        .integration(*integration)
        .program_id(crate::SVM_ALM_CONTROLLER_ID)
        .instruction()
}
