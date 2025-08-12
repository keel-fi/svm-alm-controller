use pinocchio::{
    account_info::AccountInfo, 
    msg, 
    program_error::ProgramError, 
};

use crate::{
    define_account_struct, enums::{IntegrationConfig, IntegrationState}, 
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent}, 
    integrations::utilization_market::{
        config::UtilizationMarketConfig, 
        kamino::{config::KaminoConfig, kamino_state::{Obligation, Reserve}}, 
        state::UtilizationMarketState, 
        KAMINO_LEND_PROGRAM_ID
    }, processor::SyncIntegrationAccounts, state::{Controller, Integration}
};

define_account_struct! {
    pub struct SyncKaminoAccounts<'info> {
        kamino_reserve: @owner(KAMINO_LEND_PROGRAM_ID);
        obligation: @owner(KAMINO_LEND_PROGRAM_ID);
    }
}

impl<'info> SyncKaminoAccounts<'info> {
    pub fn checked_from_accounts(
        config: &KaminoConfig,
        accounts_infos: &'info [AccountInfo]
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;

        if config.reserve.ne(ctx.kamino_reserve.key()) {
            msg! {"kamino_reserve: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        if config.obligation.ne(ctx.obligation.key()) {
            msg! {"obligation: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

pub fn process_sync_kamino(
    controller: &Controller,
    integration: &mut Integration,
    outer_ctx: &SyncIntegrationAccounts
) -> Result<(), ProgramError> {
    let config = match &integration.config {
        IntegrationConfig::UtilizationMarket(c) => {
            match c {
                UtilizationMarketConfig::KaminoConfig(kamino_config) => kamino_config,
                _ => return Err(ProgramError::InvalidAccountData),
            }
        },
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let inner_ctx = SyncKaminoAccounts::checked_from_accounts(
        config, 
        outer_ctx.remaining_accounts
    )?;

    // get the obligation data of this integration
    let obligation_data = inner_ctx.obligation.try_borrow_data()?;
    let obligation_state = Obligation::try_from(obligation_data.as_ref())?;
    obligation_state.check_from_accounts(
        outer_ctx.controller_authority.key(), 
        &config.market
    )?;

    // find the corresponding obligationCollateral to this market and its current deposited amount
    let current_collateral_amount = obligation_state
        .get_obligation_collateral_for_reserve(inner_ctx.kamino_reserve.key())
        .ok_or(ProgramError::InvalidAccountData)?
        .deposited_amount;

    // get last collateral amount saved in this integration
    let (last_deposited_liquidity_value, last_collateral_amount) = match integration.state {
        IntegrationState::UtilizationMarket(s) => {
            match s {
                UtilizationMarketState::KaminoState(state) => {
                    (state.deposited_liquidity_value, state.last_collateral_amount)
                },
                _ => return Err(ProgramError::InvalidAccountData),
            }
        },
        _ => return Err(ProgramError::InvalidAccountData),
    };

    // get the kamino reserve data
    let kamino_reserve_data = inner_ctx.kamino_reserve.try_borrow_data()?;
    let kamino_reserve_state = Reserve::try_from(kamino_reserve_data.as_ref())?;

    // calculate the value of the current liquidity deposited (using current collateral amount)
    let current_liquidity_value 
        = kamino_reserve_state.collateral_to_liquidity(current_collateral_amount);

    // emit event for change in liquidity value
    if last_deposited_liquidity_value != current_liquidity_value {
        controller.emit_event(
            outer_ctx.controller_authority, 
            outer_ctx.controller.key(), 
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: kamino_reserve_state.liquidity_mint,
                action: AccountingAction::Sync,
                before: last_deposited_liquidity_value,
                after: current_liquidity_value
            }),
        )?;
    }

    // emit event for change in collateral amount
    if last_collateral_amount != current_collateral_amount {
        controller.emit_event(
            outer_ctx.controller_authority, 
            outer_ctx.controller.key(), 
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: kamino_reserve_state.collateral_mint,
                action: AccountingAction::Sync,
                before: last_collateral_amount,
                after: current_collateral_amount
            }),
        )?;
    }

    // TODO: claim farms rewards

    // update the state
    match &mut integration.state {
        IntegrationState::UtilizationMarket(s) => {
            match s {
                UtilizationMarketState::KaminoState(kamino_state) => {
                    kamino_state.deposited_liquidity_value = current_liquidity_value;
                    kamino_state.last_collateral_amount = current_collateral_amount;
                },
                _ => return Err(ProgramError::InvalidAccountData.into()),
            }
        },
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}