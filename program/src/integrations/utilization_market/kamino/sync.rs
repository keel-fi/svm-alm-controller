use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, msg, 
    program_error::ProgramError, pubkey::Pubkey 
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::state::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED, 
    define_account_struct, 
    enums::{IntegrationConfig, IntegrationState}, 
    error::SvmAlmControllerErrors, 
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent}, 
    integrations::utilization_market::{
        config::UtilizationMarketConfig, 
        kamino::{
            constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID}, cpi::{
                derive_farm_vaults_authority, 
                derive_obligation_farm_address, 
                derive_rewards_treasury_vault, 
                derive_rewards_vault, 
                harvest_reward_cpi
            }, kamino_state::{FarmState, KaminoReserve}, shared_sync::sync_kamino_liquidity_value 
        }, 
        state::UtilizationMarketState,
    }, 
    processor::SyncIntegrationAccounts, 
    state::{Controller, Integration, Reserve}
};

define_account_struct! {
    pub struct SyncKaminoAccounts<'info> {
        vault;
        kamino_reserve: @owner(KAMINO_LEND_PROGRAM_ID);
        obligation: @owner(KAMINO_LEND_PROGRAM_ID);
        obligation_farm: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        reserve_farm: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        rewards_vault: mut;
        rewards_treasury_vault: mut;
        farm_vaults_authority;
        farms_global_config: @owner(KAMINO_FARMS_PROGRAM_ID);
        rewards_ata: mut;
        rewards_mint: @owner(pinocchio_token::ID);
        scope_prices;
        token_program: @pubkey(pinocchio_token::ID); // TODO token2022
        kamino_farms_program: @pubkey(KAMINO_FARMS_PROGRAM_ID);
        system_program: @pubkey(pinocchio_system::ID);
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
    }
}

impl<'info> SyncKaminoAccounts<'info> {
    pub fn checked_from_accounts(
        config: &IntegrationConfig,
        accounts_infos: &'info [AccountInfo],
        reserve: &Reserve,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;
        let config = match config {
            IntegrationConfig::UtilizationMarket(c) => {
                match c {
                    UtilizationMarketConfig::KaminoConfig(kamino_config) => kamino_config,
                }
            },
            _ => return Err(ProgramError::InvalidAccountData),
        };

        if config.reserve.ne(ctx.kamino_reserve.key()) {
            msg! {"kamino_reserve: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        if config.obligation.ne(ctx.obligation.key()) {
            msg! {"obligation: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        let obligation_farm_pda = derive_obligation_farm_address(
            ctx.reserve_farm.key(), 
            ctx.obligation.key(), 
            ctx.kamino_farms_program.key()
        )?;
        if obligation_farm_pda.ne(ctx.obligation_farm.key()) {
            msg! {"obligation_farm: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // validate rewards vault
        let rewards_vault_pda = derive_rewards_vault(
            ctx.reserve_farm.key(), 
            ctx.rewards_mint.key(), 
            ctx.kamino_farms_program.key()
        )?;
        if rewards_vault_pda.ne(ctx.rewards_vault.key()) {
            msg! {"rewards_vault: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // validate rewards treasury vault
        let rewards_treasury_vault_pda = derive_rewards_treasury_vault(
            ctx.farms_global_config.key(), 
            ctx.rewards_mint.key(), 
            ctx.kamino_farms_program.key()
        )?;
        if rewards_treasury_vault_pda.ne(ctx.rewards_treasury_vault.key()) {
            msg! {"rewards_treasury_vault: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // validate farm vaults authority
        let farm_vaults_authority_pda = derive_farm_vaults_authority(
            ctx.reserve_farm.key(), 
            ctx.kamino_farms_program.key()
        )?;
        if farm_vaults_authority_pda.ne(ctx.farm_vaults_authority.key()) {
            msg! {"farm_vaults_authority: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // Check consistency between the reserve
        if ctx.vault.key().ne(&reserve.vault) {
            msg! {"vault: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.vault.is_writable() {
            msg! {"vault: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if config.reserve_liquidity_mint.ne(&reserve.mint) {
            msg! {"mint: mismatch between integration configs"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

/// This function syncs a `KaminoIntegration`. This can be divided into two actions:
/// - If the kamino reserve associated with this integration has a `farm_collateral`, 
///     it harvests the rewards (through the `reward_ata` account, created if needed). 
///     If the `reward_mint` matches this integration mint, the corresponding accounting
///     event is emitted. If a Pubkey::default() `reward_ata` is passed, this action
///     is skipped.
/// - It calculates the `current_liquidity_value` based on the lp tokens held by this integration,
///     and updates the integration state.
pub fn process_sync_kamino(
    controller: &Controller,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &SyncIntegrationAccounts
) -> Result<(), ProgramError> {
    msg!("process_sync_kamino");
    let inner_ctx = SyncKaminoAccounts::checked_from_accounts(
        &integration.config, 
        outer_ctx.remaining_accounts,
        reserve
    )?;

    // Sync the reserve before main logic
    reserve.sync_balance(
        inner_ctx.vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;
    let post_sync_reserve_balance = reserve.last_balance;

    // get the kamino reserve data
    let kamino_reserve_state = {
        let kamino_reserve_data = inner_ctx.kamino_reserve.try_borrow_data()?;
        KaminoReserve::try_from(kamino_reserve_data.as_ref())?
    };

    // claim farm rewards only if the reserve has a farm collateral
    // and rewards_available > 0
    if kamino_reserve_state.has_collateral_farm() 
        && inner_ctx.rewards_ata.key().ne(&Pubkey::default())
    {
        msg!("kamino reserve has collateral_farm, processing");
        // validate that the reserve farm_collateraal matches the reserve farm
        if kamino_reserve_state.farm_collateral.ne(inner_ctx.reserve_farm.key()) {
            msg! {"reserve_farm: Invalid address"}
            return Err(ProgramError::InvalidAccountData)
        }

        // init ata if needed
        CreateIdempotent {
            funding_account: outer_ctx.authority,
            account: inner_ctx.rewards_ata,
            wallet: outer_ctx.controller_authority,
            mint: inner_ctx.rewards_mint,
            system_program: inner_ctx.system_program,
            token_program: inner_ctx.token_program
        }.invoke()?;

        // find the reward index in the FarmState of this kamino_reserve
        let reserve_farm_state = {
            let reserve_farm_data = inner_ctx.reserve_farm.try_borrow_data()?;
            FarmState::try_from(reserve_farm_data.as_ref())?
        };
        let (reward_index, rewards_available) = reserve_farm_state
            .find_reward_index_and_rewards_available(
                inner_ctx.rewards_mint.key(), 
                inner_ctx.token_program.key()
            )
            .ok_or(ProgramError::InvalidAccountData)?;

        // only harvest rewards if rewards_available > 0
        if rewards_available > 0 {
            // claim farms rewards
            harvest_reward(
                reward_index, 
                Signer::from(&[
                    Seed::from(CONTROLLER_AUTHORITY_SEED),
                    Seed::from(outer_ctx.controller.key()),
                    Seed::from(&[controller.authority_bump])
                ]), 
                outer_ctx.controller_authority, 
                &inner_ctx
            )?;

            // if there is a match between the reward_mint and the integration mint, emit event
            if inner_ctx.rewards_mint.key().eq(&reserve.mint) {
                msg!("reward mint and integration mint match, emitting accounting event");
                let post_transfer_balance = {
                    let vault = TokenAccount::from_account_info(&inner_ctx.vault)?;
                    vault.amount()
                };

                let check_delta = post_transfer_balance.saturating_sub(post_sync_reserve_balance);
                
                // Emit accounting event for debit (outflow) integration
                controller.emit_event(
                    outer_ctx.controller_authority, 
                    outer_ctx.controller.key(), 
                    SvmAlmControllerEvent::AccountingEvent(AccountingEvent { 
                        controller: *outer_ctx.controller.key(), 
                        integration: Some(*outer_ctx.integration.key()),
                        reserve: None, 
                        direction: AccountingDirection::Debit,
                        mint: *inner_ctx.rewards_mint.key(), 
                        action: AccountingAction::Withdrawal, 
                        delta: check_delta
                    })
                )?;

                // Emit accounting event for credit  (inflow) reserve
                controller.emit_event(
                    outer_ctx.controller_authority, 
                    outer_ctx.controller.key(), 
                    SvmAlmControllerEvent::AccountingEvent(AccountingEvent { 
                        controller: *outer_ctx.controller.key(), 
                        integration: None, 
                        reserve: Some(*outer_ctx.reserve.key()),
                        direction: AccountingDirection::Credit,
                        mint: *inner_ctx.rewards_mint.key(), 
                        action: AccountingAction::Withdrawal, 
                        delta: check_delta
                    })
                )?
            }
        }
    }

    // sync liquidity value and update state
    let (new_liquidity_value, new_lp_amount) = sync_kamino_liquidity_value(
        controller, 
        integration, 
        outer_ctx.integration.key(), 
        outer_ctx.controller.key(), 
        outer_ctx.controller_authority, 
        &reserve.mint, 
        inner_ctx.kamino_reserve, 
        inner_ctx.obligation
    )?;

    // update the state
    match &mut integration.state {
        IntegrationState::UtilizationMarket(s) => {
            match s {
                UtilizationMarketState::KaminoState(kamino_state) => {
                    kamino_state.last_liquidity_value = new_liquidity_value;
                    kamino_state.last_lp_amount = new_lp_amount;
                },
            }
        },
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}

fn harvest_reward(
    reward_index: u64,
    signer: Signer,
    owner: &AccountInfo,
    inner_ctx: &SyncKaminoAccounts
) -> Result<(), ProgramError> {
    harvest_reward_cpi(
        reward_index, 
        signer, 
        owner, 
        inner_ctx.obligation_farm, 
        inner_ctx.reserve_farm, 
        inner_ctx.farms_global_config, 
        inner_ctx.rewards_mint, 
        inner_ctx.rewards_ata,
        inner_ctx.rewards_vault, 
        inner_ctx.rewards_treasury_vault, 
        inner_ctx.farm_vaults_authority, 
        inner_ctx.scope_prices, 
        inner_ctx.token_program, 
        inner_ctx.kamino_farms_program
    )?;

    Ok(())
}