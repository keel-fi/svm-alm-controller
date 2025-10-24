/* Trait-based LendingProcessor for dramatic code reduction */

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};

use crate::{
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    state::{Controller, Integration, Permission, Reserve},
};

/// Result of a lending operation
#[derive(Debug)]
pub struct LendingOperationResult {
    pub integration_delta: u64,
    pub reserve_delta: u64,
    pub new_balance: u64,
}

/// Context for lending operations
pub struct LendingContext<'info> {
    pub controller: &'info Controller,
    pub permission: &'info Permission,
    pub integration: &'info mut Integration,
    pub reserve: &'info mut Reserve,
    pub controller_authority: &'info AccountInfo,
    pub controller_pubkey: &'info Pubkey,
    pub integration_pubkey: &'info Pubkey,
    pub reserve_pubkey: &'info Pubkey,
    pub mint: &'info Pubkey,
}

/// Trait for lending protocol processors
/// This trait abstracts away all the common lending operations
pub trait LendingProcessor {
    /// Get the current balance from the lending protocol
    fn get_current_balance(&self, ctx: &LendingContext) -> Result<u64, ProgramError>;
    
    /// Update the integration state with new balance
    fn update_integration_state(&self, integration: &mut Integration, new_balance: u64) -> Result<(), ProgramError>;
    
    /// Execute a deposit operation
    fn execute_deposit(&self, ctx: &mut LendingContext, amount: u64) -> Result<LendingOperationResult, ProgramError>;
    
    /// Execute a withdrawal operation  
    fn execute_withdrawal(&self, ctx: &mut LendingContext, amount: u64) -> Result<LendingOperationResult, ProgramError>;
    
    /// Sync the lending protocol balance (for interest accrual)
    fn sync_balance(&self, ctx: &mut LendingContext) -> Result<u64, ProgramError>;
    
    /// Get the reserve vault account info
    fn get_reserve_vault(&self, ctx: &LendingContext) -> Result<&AccountInfo, ProgramError>;
}

/// Generic lending operation handler that works with any LendingProcessor
pub fn process_lending_push<P: LendingProcessor>(
    processor: &P,
    ctx: &mut LendingContext,
    amount: u64,
) -> ProgramResult {
    // Validate permissions
    if !ctx.permission.can_reallocate() {
        return Err(ProgramError::IncorrectAuthority);
    }

    // Sync reserve balance
    let reserve_vault = processor.get_reserve_vault(ctx)?;
    ctx.reserve.sync_balance(
        reserve_vault,
        ctx.controller_authority,
        ctx.controller_pubkey,
        ctx.controller,
    )?;

    // Sync lending protocol balance
    processor.sync_balance(ctx)?;

    // Execute deposit
    let result = processor.execute_deposit(ctx, amount)?;

    // Emit accounting events
    emit_double_accounting_events(
        ctx,
        result.integration_delta,
        result.reserve_delta,
        AccountingAction::Deposit,
    )?;

    // Update integration state
    processor.update_integration_state(ctx.integration, result.new_balance)?;

    // Update rate limits
    update_rate_limits(ctx, result.reserve_delta, false)?;

    Ok(())
}

/// Generic lending pull operation handler
pub fn process_lending_pull<P: LendingProcessor>(
    processor: &P,
    ctx: &mut LendingContext,
    amount: u64,
) -> ProgramResult {
    // Validate permissions
    if !ctx.permission.can_reallocate() && !ctx.permission.can_liquidate(ctx.integration) {
        return Err(ProgramError::IncorrectAuthority);
    }

    // Sync reserve balance
    let reserve_vault = processor.get_reserve_vault(ctx)?;
    ctx.reserve.sync_balance(
        reserve_vault,
        ctx.controller_authority,
        ctx.controller_pubkey,
        ctx.controller,
    )?;

    // Sync lending protocol balance
    processor.sync_balance(ctx)?;

    // Execute withdrawal
    let result = processor.execute_withdrawal(ctx, amount)?;

    // Emit accounting events
    emit_double_accounting_events(
        ctx,
        result.integration_delta,
        result.reserve_delta,
        AccountingAction::Withdrawal,
    )?;

    // Update integration state
    processor.update_integration_state(ctx.integration, result.new_balance)?;

    // Update rate limits
    update_rate_limits(ctx, result.reserve_delta, true)?;

    Ok(())
}

/// Generic lending sync operation handler
pub fn process_lending_sync<P: LendingProcessor>(
    processor: &P,
    ctx: &mut LendingContext,
) -> ProgramResult {
    // Sync reserve balance
    let reserve_vault = processor.get_reserve_vault(ctx)?;
    ctx.reserve.sync_balance(
        reserve_vault,
        ctx.controller_authority,
        ctx.controller_pubkey,
        ctx.controller,
    )?;

    // Sync lending protocol balance
    let new_balance = processor.sync_balance(ctx)?;

    // Update integration state
    processor.update_integration_state(ctx.integration, new_balance)?;

    Ok(())
}

/// Helper to emit double accounting events
fn emit_double_accounting_events(
    ctx: &LendingContext,
    integration_delta: u64,
    reserve_delta: u64,
    action: AccountingAction,
) -> ProgramResult {
    let (integration_direction, reserve_direction) = match action {
        AccountingAction::Deposit => (AccountingDirection::Credit, AccountingDirection::Debit),
        AccountingAction::Withdrawal => (AccountingDirection::Debit, AccountingDirection::Credit),
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Emit accounting event for integration
    ctx.controller.emit_event(
        ctx.controller_authority,
        ctx.controller_pubkey,
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *ctx.controller_pubkey,
            integration: Some(*ctx.integration_pubkey),
            mint: *ctx.mint,
            reserve: None,
            direction: integration_direction,
            action: action.clone(),
            delta: integration_delta,
        }),
    )?;

    // Emit accounting event for reserve (double accounting)
    ctx.controller.emit_event(
        ctx.controller_authority,
        ctx.controller_pubkey,
        SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
            controller: *ctx.controller_pubkey,
            integration: None,
            mint: *ctx.mint,
            reserve: Some(*ctx.reserve_pubkey),
            direction: reserve_direction,
            action,
            delta: reserve_delta,
        }),
    )?;

    Ok(())
}

/// Helper to update rate limits
fn update_rate_limits(
    ctx: &mut LendingContext,
    amount: u64,
    is_inflow: bool,
) -> ProgramResult {
    use pinocchio::sysvars::{clock::Clock, Sysvar};
    
    let clock = Clock::get()?;
    
    if is_inflow {
        ctx.integration.update_rate_limit_for_inflow(clock, amount)?;
        ctx.reserve.update_for_inflow(clock, amount)?;
    } else {
        ctx.integration.update_rate_limit_for_outflow(clock, amount)?;
        ctx.reserve.update_for_outflow(clock, amount, false)?;
    }
    
    Ok(())
}
