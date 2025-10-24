use pinocchio::{
    msg,
    program_error::ProgramError,
    ProgramResult,
};

use crate::{
    instructions::PushArgs,
    integrations::{
        drift::lending_processor::DriftLendingProcessor,
        shared::lending_processor::{LendingContext, process_lending_push},
    },
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

pub fn process_push_drift(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> ProgramResult {
    msg!("process_push_drift");

    let (market_index, amount) = match outer_args {
        PushArgs::Drift {
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

    // Use generic lending push handler
    process_lending_push(&processor, &mut lending_ctx, amount)
}
