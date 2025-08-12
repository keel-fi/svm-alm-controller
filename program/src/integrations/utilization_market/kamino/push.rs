use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, msg, 
    program_error::ProgramError, 
    pubkey::Pubkey, 
    sysvars::{clock::Clock, instructions::INSTRUCTIONS_ID, Sysvar}
};
use pinocchio_token::state::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED, 
    define_account_struct, 
    enums::{IntegrationConfig, IntegrationState}, 
    events::{AccountingAction, AccountingEvent, SvmAlmControllerEvent}, 
    instructions::PushArgs, integrations::utilization_market::{
        config::UtilizationMarketConfig, kamino::cpi::{
            deposit_reserve_liquidity_v2_cpi, 
            derive_market_authority_address, 
            derive_reserve_collateral_mint, 
            derive_reserve_collateral_supply, 
            derive_reserve_liquidity_supply
        }, 
        state::UtilizationMarketState, 
        KAMINO_FARMS_PROGRAM_ID, 
        KAMINO_LEND_PROGRAM_ID
    }, processor::PushAccounts, 
    state::{Controller, Integration, Permission, Reserve}
};

define_account_struct! {
    pub struct PushKaminoAccounts<'info> {
        liquidity_source: mut @owner(pinocchio_token::ID); // TODO: token 2022 support
        obligation: mut @owner(KAMINO_LEND_PROGRAM_ID);
        reserve: mut @owner(KAMINO_LEND_PROGRAM_ID);
        reserve_liquidity_mint: @owner(pinocchio_token::ID); // TODO: token 2022 support
        reserve_liquidity_supply: mut @owner(pinocchio_token::ID); // TODO: token 2022 support
        reserve_collateral_mint: mut @owner(pinocchio_token::ID); // TODO: token 2022 support
        reserve_collateral_supply: mut @owner(pinocchio_token::ID); // TODO: token 2022 support
        market_authority;
        market: @owner(KAMINO_LEND_PROGRAM_ID);
        collateral_token_program: @pubkey(pinocchio_token::ID); // TODO: token 2022 support
        liquidity_token_program: @pubkey(pinocchio_token::ID); // TODO: token 2022 support
        instruction_sysvar_account: @pubkey(INSTRUCTIONS_ID);
        obligation_farm_collateral: mut;
        reserve_farm_collateral: mut;
        kamino_farms_program: @pubkey(KAMINO_FARMS_PROGRAM_ID);
        kamino_program: @pubkey(KAMINO_LEND_PROGRAM_ID);
    }
}

impl<'info> PushKaminoAccounts<'info> {
    pub fn checked_from_accounts(
        controller_authority: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::UtilizationMarket(c) => {
                match c {
                    UtilizationMarketConfig::KaminoConfig(kamino_config) => kamino_config,
                    _ => return Err(ProgramError::InvalidAccountData),
                }
            },
            _ => return Err(ProgramError::InvalidAccountData),
        };

        if ctx.market.key().ne(&config.market) {
            msg! {"market: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.reserve.key().ne(&config.reserve) {
            msg! {"reserve: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.reserve_farm_collateral.key().ne(&config.reserve_farm_collateral) {
            msg! {"reserve_farm_collateral: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.reserve_liquidity_mint.key().ne(&config.reserve_liquidity_mint) {
            msg! {"reserve_liquidity_mint: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        let reserve_collateral_mint_address = derive_reserve_collateral_mint(
            &ctx.market.key(), 
            &ctx.reserve_liquidity_mint.key(), 
            &KAMINO_LEND_PROGRAM_ID
        );
        if ctx.reserve_collateral_mint.key().ne(&reserve_collateral_mint_address) {
            msg! {"reserve_collateral_mint: does not match PDA"};
            return Err(ProgramError::InvalidAccountData);
        }

        let reserve_collateral_supply_address = derive_reserve_collateral_supply(
            &ctx.market.key(), 
            &ctx.reserve_liquidity_mint.key(), 
            &KAMINO_LEND_PROGRAM_ID
        );
        if ctx.reserve_collateral_supply.key().ne(&reserve_collateral_supply_address) {
            msg! {"reserve_collateral_supply: does not match PDA"};
            return Err(ProgramError::InvalidAccountData);
        }

        let reserve_liquidity_supply_address = derive_reserve_liquidity_supply(
            &ctx.market.key(), 
            &ctx.reserve_liquidity_mint.key(), 
            &KAMINO_LEND_PROGRAM_ID
        );
        if ctx.reserve_liquidity_supply.key().ne(&reserve_liquidity_supply_address) {
            msg! {"reserve_liquidity_supply: does not match PDA"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.obligation.key().ne(&config.obligation) {
            msg! {"obligation: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        let market_authority_pda = derive_market_authority_address(
            ctx.market.key(), 
            &KAMINO_LEND_PROGRAM_ID
        );
        if &market_authority_pda != ctx.market_authority.key() {
            msg! {"market authority: Invalid address"}
            return Err(ProgramError::InvalidSeeds)
        }

        let liquidity_source_token_account 
            = TokenAccount::from_account_info(ctx.liquidity_source)?;
        if liquidity_source_token_account.mint().ne(&config.reserve_liquidity_mint) {
            msg! {"liquidity_source: invalid mint"};
            return Err(ProgramError::InvalidAccountData);
        }
        if liquidity_source_token_account.owner().ne(controller_authority) {
            msg! {"liquidity_source: not owned by Controller authority PDA"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

pub fn process_push_kamino(
    controller: &Controller,
    permission: &Permission,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &PushAccounts,
    outer_args: &PushArgs,
) -> Result<(), ProgramError> {
    msg!("process_push_kamino");

    // Get the current slot and time
    let clock = Clock::get()?;
    
    let amount = match outer_args {
        PushArgs::Kamino { amount } => *amount,
        _ => return Err(ProgramError::InvalidAccountData),
    };

    if amount == 0 {
        msg! {"amount must be > 0"};
        return Err(ProgramError::InvalidArgument);
    }

    if !permission.can_reallocate() {
        msg! {"permission: can_reallocate required"};
        return Err(ProgramError::IncorrectAuthority);
    }

    let inner_ctx = PushKaminoAccounts::checked_from_accounts(
        outer_ctx.controller_authority.key(), 
        &integration.config, 
        outer_ctx.remaining_accounts
    )?;

    // Check against reserve data
    if inner_ctx.liquidity_source.key().ne(&reserve.vault) {
        msg! {"liquidity_source: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData);
    }

    if inner_ctx.reserve_liquidity_mint.key().ne(&reserve.mint) {
        msg! {"reserve_liquidity_mint: mismatch with reserve"};
        return Err(ProgramError::InvalidAccountData)
    }

    reserve.sync_balance(
        inner_ctx.liquidity_source,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;

    // TODO: Sync events

    // for liquidity amount calculation
    let liquidity_source_account = TokenAccount::from_account_info(inner_ctx.liquidity_source)?;
    let liquidity_amount_before = liquidity_source_account.amount();
    drop(liquidity_source_account);

    // for collateral amount calculation
    let collateral_destination_account = TokenAccount::from_account_info(inner_ctx.reserve_collateral_supply)?;
    let collateral_amount_before = collateral_destination_account.amount();
    drop(collateral_destination_account);

    // perform kamino deposit liquidity cpi
    deposit_reserve_liquidity_v2(
        amount, 
        Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ]), 
        outer_ctx.controller_authority, 
        &inner_ctx
    )?;

    // for liquidity amount calculation
    let liquidity_source_account = TokenAccount::from_account_info(inner_ctx.liquidity_source)?;
    let liquidity_amount_after = liquidity_source_account.amount();
    drop(liquidity_source_account);

    let final_liquidity_amount = liquidity_amount_before.checked_sub(liquidity_amount_after)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    // for collateral amount calculation
    let collateral_destination_account = TokenAccount::from_account_info(inner_ctx.reserve_collateral_supply)?;
    let collateral_amount_after = collateral_destination_account.amount();
    drop(collateral_destination_account);

    // collateral is minted (increases)
    let final_collateral_amount = collateral_amount_after.checked_sub(collateral_amount_before)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    // emit accounting event
    if final_liquidity_amount > 0 {
        controller.emit_event(
            outer_ctx.controller_authority,
            outer_ctx.controller.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: *inner_ctx.reserve_liquidity_mint.key(),
                action: AccountingAction::Withdrawal, // TODO: is it withdrawal or deposit?
                before: liquidity_amount_before,
                after: liquidity_amount_after,
            }),
        )?;
    }

    // emit accounting event
    if final_collateral_amount > 0 {
        controller.emit_event(
            outer_ctx.controller_authority,
            outer_ctx.controller.key(),
            SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                controller: *outer_ctx.controller.key(),
                integration: *outer_ctx.integration.key(),
                mint: *inner_ctx.reserve_collateral_mint.key(),
                action: AccountingAction::Deposit, // TODO: is it deposit or withdrawal?
                before: collateral_amount_before,
                after: collateral_amount_after,
            }),
        )?;
    }

    // update the state
    match &mut integration.state {
        IntegrationState::UtilizationMarket(state) => {
            match state {
                UtilizationMarketState::KaminoState(kamino_state) => {
                    kamino_state.deposited_liquidity_value += final_liquidity_amount;
                    kamino_state.last_collateral_amount += final_collateral_amount;
                }
                _ => return Err(ProgramError::InvalidAccountData.into()),
            }
        },
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    // update the integration rate limit for outflow
    integration.update_rate_limit_for_outflow(clock, final_liquidity_amount)?;

    // update the reserves for the flows
    reserve.update_for_outflow(clock, final_liquidity_amount)?;

    Ok(())
}


fn deposit_reserve_liquidity_v2(
    amount: u64,
    signer: Signer,
    owner: &AccountInfo,
    inner_ctx: &PushKaminoAccounts
) -> Result<(), ProgramError> {
    deposit_reserve_liquidity_v2_cpi(
        amount, 
        signer,
        owner, 
        inner_ctx.obligation, 
        inner_ctx.market, 
        inner_ctx.market_authority, 
        inner_ctx.reserve, 
        inner_ctx.reserve_liquidity_mint, 
        inner_ctx.reserve_liquidity_supply, 
        inner_ctx.reserve_collateral_mint, 
        inner_ctx.reserve_collateral_supply, 
        inner_ctx.liquidity_source, 
        inner_ctx.collateral_token_program, 
        inner_ctx.liquidity_token_program, 
        inner_ctx.instruction_sysvar_account, 
        inner_ctx.obligation_farm_collateral, 
        inner_ctx.reserve_farm_collateral, 
        inner_ctx.kamino_farms_program, 
        inner_ctx.kamino_program, 
    )?;

    Ok(())
}