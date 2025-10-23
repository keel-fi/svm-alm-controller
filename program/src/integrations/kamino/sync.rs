use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
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
        protocol_state::{FarmState, KaminoReserve, UserState},
        shared_sync::sync_kamino_liquidity_value,
    },
    processor::SyncIntegrationAccounts,
    state::{Controller, Integration, Reserve},
};

define_account_struct! {
    pub struct SyncKaminoAccounts<'info> {
        reserve_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        kamino_reserve: @owner(KAMINO_LEND_PROGRAM_ID);
        obligation: @owner(KAMINO_LEND_PROGRAM_ID);
        // This account has a custom check since it's an optional account.
        obligation_farm: mut;
        // This account has a custom check since reserve_farm_collateral
        // can be equal to Pubkey::default() if the kamino_reserve has no farm.
        kamino_reserve_farm: mut;
        // validated only if obligation_farm != Pubkey::default()
        rewards_vault: mut;
        // validated only if obligation_farm != Pubkey::default()
        rewards_treasury_vault: mut;
        // validated only if obligation_farm != Pubkey::default()
        farm_vaults_authority;
        farms_global_config: @owner(KAMINO_FARMS_PROGRAM_ID);
        // validated only if obligation_farm != Pubkey::default()
        rewards_ata: mut;
        // validated only if obligation_farm != Pubkey::default()
        rewards_mint;
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

        // We only perform farm and rewards validations if the kamino_reserve_farm
        // is not Pubkey::default (meaning the kamino_reserve actually has a farm and rewards can be harvested)
        if ctx.kamino_reserve_farm.key().ne(&Pubkey::default()) {
            // validate kamino_reserve_farm is owned by KFARMS
            if !ctx
                .kamino_reserve_farm
                .is_owned_by(&KAMINO_FARMS_PROGRAM_ID)
            {
                msg! {"kamino_reserve_farm: invalid owner"};
                return Err(ProgramError::InvalidAccountOwner);
            }

            // rewards_ata can either be pubkey::default or be owned by spl_token/token2022 programs
            if ctx.rewards_ata.key().ne(&Pubkey::default())
                && !ctx.rewards_ata.is_owned_by(&pinocchio_token::ID)
                && !ctx.rewards_ata.is_owned_by(&pinocchio_token2022::ID)
            {
                msg! {"rewards_ata: Invalid owner"}
                return Err(ProgramError::IllegalOwner);
            }

            // validate obligation_farm
            if !ctx.obligation_farm.is_owned_by(&KAMINO_FARMS_PROGRAM_ID) {
                msg! {"obligation_farm: invalid owner"};
                return Err(ProgramError::InvalidAccountOwner);
            }
            let obligation_farm_pda = derive_obligation_farm_address(
                ctx.kamino_reserve_farm.key(),
                ctx.obligation.key(),
                ctx.kamino_farms_program.key(),
            )?;
            if obligation_farm_pda.ne(ctx.obligation_farm.key()) {
                msg! {"obligation_farm: Invalid address"}
                return Err(SvmAlmControllerErrors::InvalidPda.into());
            }

            // Validate rewards mint
            if !ctx.rewards_mint.is_owned_by(&pinocchio_token::ID)
                && !ctx.rewards_mint.is_owned_by(&pinocchio_token2022::ID)
            {
                msg! {"rewards_mint: invalid owner"};
                return Err(ProgramError::InvalidAccountOwner);
            }

            // Validate rewards vault
            if !ctx.rewards_vault.is_owned_by(&pinocchio_token::ID)
                && !ctx.rewards_vault.is_owned_by(&pinocchio_token2022::ID)
            {
                msg! {"rewards_vault: invalid owner"};
                return Err(ProgramError::InvalidAccountOwner);
            }
            let rewards_vault_pda = derive_rewards_vault(
                ctx.kamino_reserve_farm.key(),
                ctx.rewards_mint.key(),
                ctx.kamino_farms_program.key(),
            )?;
            if rewards_vault_pda.ne(ctx.rewards_vault.key()) {
                msg! {"rewards_vault: Invalid address"}
                return Err(SvmAlmControllerErrors::InvalidPda.into());
            }

            // Validate rewards treasury vault
            if !ctx.rewards_treasury_vault.is_owned_by(&pinocchio_token::ID)
                && !ctx
                    .rewards_treasury_vault
                    .is_owned_by(&pinocchio_token2022::ID)
            {
                msg! {"rewards_treasury_vault: invalid owner"};
                return Err(ProgramError::InvalidAccountOwner);
            }
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
            let farm_vaults_authority_pda = derive_farm_vaults_authority(
                ctx.kamino_reserve_farm.key(),
                ctx.kamino_farms_program.key(),
            )?;
            if farm_vaults_authority_pda.ne(ctx.farm_vaults_authority.key()) {
                msg! {"farm_vaults_authority: Invalid address"}
                return Err(SvmAlmControllerErrors::InvalidPda.into());
            }
        }

        // Check consistency between the reserve
        // the reserve.mint is being checked in config.check_accounts
        if ctx.reserve_vault.key().ne(&reserve.vault) {
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
        inner_ctx.reserve_vault,
        outer_ctx.controller_authority,
        outer_ctx.controller.key(),
        controller,
    )?;
    let post_sync_reserve_balance = reserve.last_balance;

    // Get the kamino reserve state
    let kamino_reserve_data = inner_ctx.kamino_reserve.try_borrow_data()?;
    let kamino_reserve_state = KaminoReserve::load_checked(&kamino_reserve_data)?;

    // Claim farm rewards only if the reserve has a farm collateral
    // and rewards_available > 0
    if kamino_reserve_state.has_collateral_farm()
        && inner_ctx.rewards_ata.key().ne(&Pubkey::default())
    {
        // Validate that the reserve farm_collateral matches the reserve farm
        if kamino_reserve_state
            .farm_collateral
            .ne(inner_ctx.kamino_reserve_farm.key())
        {
            msg! {"reserve_farm: Invalid address"}
            return Err(ProgramError::InvalidAccountData);
        }

        // Find the reward index in the FarmState of this kamino_reserve
        let (reward_index, rewards_available) = {
            let reserve_farm_data = inner_ctx.kamino_reserve_farm.try_borrow_data()?;
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
                farm_state: inner_ctx.kamino_reserve_farm,
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
                let post_transfer_balance = {
                    let vault = TokenAccount::from_account_info(&inner_ctx.reserve_vault)?;
                    vault.amount()
                };

                let check_delta = post_transfer_balance.saturating_sub(post_sync_reserve_balance);

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
                        delta: check_delta,
                    }),
                )?
            }
        }
    }

    // Sync liquidity value and update state
    let (new_liquidity_value, new_lp_amount) = sync_kamino_liquidity_value(
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
        IntegrationState::Kamino(kamino_state) => {
            kamino_state.last_liquidity_value = new_liquidity_value;
            kamino_state.last_lp_amount = new_lp_amount;
        }
        _ => return Err(ProgramError::InvalidAccountData.into()),
    }

    Ok(())
}
