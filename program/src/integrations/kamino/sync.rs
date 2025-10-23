use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    events::{AccountingAction, AccountingDirection, AccountingEvent, SvmAlmControllerEvent},
    integrations::kamino::{
        constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID},
        cpi::HarvestReward,
        pdas::{
            derive_farm_vaults_authority, derive_obligation_farm_address,
            derive_rewards_treasury_vault, derive_rewards_vault,
        },
        protocol_state::{FarmState, KaminoReserve, PriceStatusFlags, UserState},
        shared_sync::sync_kamino_liquidity_value,
    },
    processor::SyncIntegrationAccounts,
    state::{Controller, Integration, Reserve},
};

define_account_struct! {
    pub struct SyncKaminoAccounts<'info> {
        vault: mut;
        kamino_reserve: @owner(KAMINO_LEND_PROGRAM_ID);
        obligation: @owner(KAMINO_LEND_PROGRAM_ID);
        obligation_farm: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        reserve_farm: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        rewards_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        rewards_treasury_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        farm_vaults_authority;
        farms_global_config: @owner(KAMINO_FARMS_PROGRAM_ID);
        rewards_ata: mut;
        rewards_mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        scope_prices;
        rewards_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
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
            IntegrationConfig::Kamino(kamino_config) => kamino_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        config.check_accounts(
            ctx.obligation.key(),
            ctx.kamino_reserve.key(),
            &reserve.mint,
            None,
        )?;

        // rewards_ata can either be pubkey::default or be owned by spl_token/token2022 programs
        if ctx.rewards_ata.key().ne(&Pubkey::default())
            && !ctx.rewards_ata.is_owned_by(&pinocchio_token::ID)
            && !ctx.rewards_ata.is_owned_by(&pinocchio_token2022::ID)
        {
            msg! {"rewards_ata: Invalid owner"}
            return Err(ProgramError::IllegalOwner);
        }

        let obligation_farm_pda = derive_obligation_farm_address(
            ctx.reserve_farm.key(),
            ctx.obligation.key(),
            ctx.kamino_farms_program.key(),
        )?;
        if obligation_farm_pda.ne(ctx.obligation_farm.key()) {
            msg! {"obligation_farm: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // Validate rewards vault
        let rewards_vault_pda = derive_rewards_vault(
            ctx.reserve_farm.key(),
            ctx.rewards_mint.key(),
            ctx.kamino_farms_program.key(),
        )?;
        if rewards_vault_pda.ne(ctx.rewards_vault.key()) {
            msg! {"rewards_vault: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // Validate rewards treasury vault
        let rewards_treasury_vault_pda = derive_rewards_treasury_vault(
            ctx.farms_global_config.key(),
            ctx.rewards_mint.key(),
            ctx.kamino_farms_program.key(),
        )?;
        if rewards_treasury_vault_pda.ne(ctx.rewards_treasury_vault.key()) {
            msg! {"rewards_treasury_vault: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // Validate farm vaults authority
        let farm_vaults_authority_pda =
            derive_farm_vaults_authority(ctx.reserve_farm.key(), ctx.kamino_farms_program.key())?;
        if farm_vaults_authority_pda.ne(ctx.farm_vaults_authority.key()) {
            msg! {"farm_vaults_authority: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // Check consistency between the reserve
        // the reserve.mint is being checked in config.check_accounts
        if ctx.vault.key().ne(&reserve.vault) {
            msg! {"vault: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

/// This function syncs a `KaminoIntegration`. This can be divided into two actions:
/// - If the kamino reserve associated with this integration has a `farm_collateral`,
///     it harvests the rewards (through the `rewards_ata` account, created if needed).
///     If the `reward_mint` matches this integration mint, the corresponding accounting
///     event is emitted. If a Pubkey::default() `reward_ata` is passed, this action
///     is skipped.
/// - It calculates the `current_liquidity_value` based on the lp tokens held by this integration,
///     and updates the integration state.
pub fn process_sync_kamino(
    controller: &Controller,
    integration: &mut Integration,
    reserve: &mut Reserve,
    outer_ctx: &SyncIntegrationAccounts,
) -> Result<(), ProgramError> {
    msg!("process_sync_kamino");
    let inner_ctx = SyncKaminoAccounts::checked_from_accounts(
        &integration.config,
        outer_ctx.remaining_accounts,
        reserve,
    )?;

    // Sync the reserve before main logic
    reserve.sync_balance(
        inner_ctx.vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;
    let reserve_vault_balance_before = reserve.last_balance;

    // Get the kamino reserve state
    let kamino_reserve_data = inner_ctx.kamino_reserve.try_borrow_data()?;
    let kamino_reserve_state = KaminoReserve::load_checked(&kamino_reserve_data)?;

    let clock = Clock::get()?;
    // PriceStatusFlags::NONE means that no price checks are required
    // This is because we do not have borrows for this integration yet.
    // If we have borrows, we will need to check the price status flags ie PriceStatusFlags::ALL_CHECKS.
    if kamino_reserve_state
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg! {"kamino_reserve: is stale and must be refreshed in the current slot"}
        return Err(ProgramError::InvalidAccountData);
    }

    // Claim farm rewards only if the reserve has a farm collateral
    // and rewards_available > 0
    if kamino_reserve_state.has_collateral_farm()
        && inner_ctx.rewards_ata.key().ne(&Pubkey::default())
    {
        // Validate that the reserve farm_collateral matches the reserve farm
        if kamino_reserve_state
            .farm_collateral
            .ne(inner_ctx.reserve_farm.key())
        {
            msg! {"reserve_farm: Invalid address"}
            return Err(ProgramError::InvalidAccountData);
        }

        // Find the reward index in the FarmState of this kamino_reserve
        let (reward_index, rewards_available) = {
            let reserve_farm_data = inner_ctx.reserve_farm.try_borrow_data()?;
            let reserve_farm_state = FarmState::load_checked(&reserve_farm_data)?;
            reserve_farm_state
                .find_reward_index_and_rewards_available(
                    inner_ctx.rewards_mint.key(),
                    inner_ctx.rewards_token_program.key(),
                )
                .ok_or(ProgramError::InvalidAccountData)?
        };

        // Only harvest rewards if rewards_available > 0
        if rewards_available > 0 {
            // Get available rewards in obligation farm before harvesting rewards
            let user_rewards = {
                UserState::get_rewards(
                    inner_ctx.obligation_farm,
                    inner_ctx.farms_global_config,
                    reward_index as usize,
                )?
            };

            // Initialize ATA if needed
            CreateIdempotent {
                funding_account: outer_ctx.payer,
                account: inner_ctx.rewards_ata,
                wallet: outer_ctx.controller_authority,
                mint: inner_ctx.rewards_mint,
                system_program: inner_ctx.system_program,
                token_program: inner_ctx.rewards_token_program,
            }
            .invoke()?;

            // Claim farms rewards
            HarvestReward {
                owner: outer_ctx.controller_authority,
                user_state: inner_ctx.obligation_farm,
                farm_state: inner_ctx.reserve_farm,
                global_config: inner_ctx.farms_global_config,
                reward_mint: inner_ctx.rewards_mint,
                user_reward_ata: inner_ctx.rewards_ata,
                rewards_vault: inner_ctx.rewards_vault,
                rewards_treasure_vault: inner_ctx.rewards_treasury_vault,
                farm_vaults_authority: inner_ctx.farm_vaults_authority,
                scope_prices: inner_ctx.scope_prices,
                token_program: inner_ctx.rewards_token_program,
                reward_index,
            }
            .invoke_signed(&[Signer::from(&[
                Seed::from(CONTROLLER_AUTHORITY_SEED),
                Seed::from(outer_ctx.controller.key()),
                Seed::from(&[controller.authority_bump]),
            ])])?;

            // If there is a match between the reward_mint and the integration mint, emit event
            if inner_ctx.rewards_mint.key().eq(&reserve.mint) {
                let reserve_vault_balance_after = {
                    let vault = TokenAccount::from_account_info(&inner_ctx.vault)?;
                    vault.amount()
                };

                let reserve_vault_balance_delta =
                    reserve_vault_balance_after.saturating_sub(reserve_vault_balance_before);

                // Emit sync accounting event for credit (inflow) integration
                controller.emit_event(
                    outer_ctx.controller_authority,
                    outer_ctx.controller.key(),
                    SvmAlmControllerEvent::AccountingEvent(AccountingEvent {
                        controller: *outer_ctx.controller.key(),
                        integration: Some(*outer_ctx.integration.key()),
                        reserve: None,
                        direction: AccountingDirection::Credit,
                        mint: *inner_ctx.rewards_mint.key(),
                        action: AccountingAction::Sync,
                        delta: user_rewards,
                    }),
                )?;

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
                        delta: user_rewards,
                    }),
                )?;

                // Emit accounting event for credit (inflow) reserve
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
                        delta: reserve_vault_balance_delta,
                    }),
                )?
            }
        }
    }

    // Sync Integration balance
    let new_balance = sync_kamino_liquidity_value(
        controller,
        integration,
        outer_ctx.integration.key(),
        outer_ctx.controller.key(),
        outer_ctx.controller_authority,
        &reserve.mint,
        inner_ctx.kamino_reserve,
        inner_ctx.obligation,
    )?;

    // Update the state
    match &mut integration.state {
        IntegrationState::Kamino(state) => {
            state.balance = new_balance;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}
