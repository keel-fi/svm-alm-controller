use pinocchio::{
    msg,
    program_error::ProgramError,
    ProgramResult,
};

use crate::{
    instructions::PullArgs,
    integrations::{
        drift::lending_processor::DriftLendingProcessor,
        shared::lending_processor::{LendingContext, process_lending_pull},
    },
    processor::PullAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

/// This function performs a "Pull" on a `DriftIntegration`.
/// Invokes Drift Withdraw instruction
pub fn process_pull_drift(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PullAccounts,
    outer_args: &PullArgs,
) -> ProgramResult {
    msg!("process_pull_drift");

    let (market_index, amount) = match outer_args {
        PullArgs::Drift {
            market_index,
            amount,
        } => (*market_index, *amount),
        _ => return Err(ProgramError::InvalidArgument),
    };

    if amount == 0 {
        msg! {"amount: must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    // Store mint to avoid borrowing issues
    let mint = reserve.mint;
    
    // Create lending context
    let mut lending_ctx = LendingContext {
        controller,
        permission,
        integration,
        reserve,
        controller_authority: outer_ctx.controller_authority,
        controller_pubkey: outer_ctx.controller.key(),
        integration_pubkey: outer_ctx.integration.key(),
        reserve_pubkey: outer_ctx.reserve_a.key(),
        mint: &mint,
    };

    // Create drift processor
    let processor = DriftLendingProcessor {
        market_index,
    };

    // Use generic lending pull handler
    process_lending_pull(&processor, &mut lending_ctx, amount)
}
