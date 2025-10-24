use pinocchio::{
    msg,
    program_error::ProgramError,
};

use crate::{
    instructions::PushArgs,
    integrations::{
        kamino::lending_processor::KaminoLendingProcessor,
        shared::lending_processor::{LendingContext, process_lending_push},
    },
    processor::PushAccounts,
    state::{Controller, Integration, Permission, Reserve},
};

/// This function performs a "Push" on a `KaminoIntegration`.
pub fn process_push_kamino(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    msg!("process_push_kamino");

    let amount = match outer_args {
        PushArgs::Kamino { amount } => *amount,
        _ => return Err(ProgramError::InvalidArgument),
    };

    if amount == 0 {
        msg! {"amount must be > 0"};
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

    // Create kamino processor
    let processor = KaminoLendingProcessor;

    // Use generic lending push handler
    process_lending_push(&processor, &mut lending_ctx, amount)
}
