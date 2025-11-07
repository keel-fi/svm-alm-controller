use account_zerocopy_deserialize::AccountZerocopyDeserialize;
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
        kfarms_protocol_state::FarmState,
        klend_protocol_state::{KaminoReserve, PriceStatusFlags},
        pdas::{
            derive_farm_vaults_authority, derive_obligation_farm_address,
            derive_rewards_treasury_vault, derive_rewards_vault,
        },
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
        @remaining_accounts as remaining_accounts;
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

        // Check consistency between the reserve
        // the reserve.mint is being checked in config.check_accounts
        if ctx.reserve_vault.key().ne(&reserve.vault) {
            msg! {"vault: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}

define_account_struct! {
    pub struct HarvestKaminoAccounts<'info> {
        obligation_farm: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        kamino_reserve_farm: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        rewards_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        rewards_treasury_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        farm_vaults_authority;
        farms_global_config: @owner(KAMINO_FARMS_PROGRAM_ID);
        rewards_ata: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID, pinocchio_system::ID);
        rewards_mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        scope_prices;
        rewards_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        kamino_farms_program: @pubkey(KAMINO_FARMS_PROGRAM_ID);
        system_program: @pubkey(pinocchio_system::ID);
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
    }
}

impl<'info> HarvestKaminoAccounts<'info> {
    pub fn checked_from_accounts(
        obligation: &Pubkey,
        accounts_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(accounts_infos)?;

        // validate obligation_farm
        let obligation_farm_pda = derive_obligation_farm_address(
            ctx.kamino_reserve_farm.key(),
            obligation,
            ctx.kamino_farms_program.key(),
        )?;
        if obligation_farm_pda.ne(ctx.obligation_farm.key()) {
            msg! {"obligation_farm: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // Validate rewards vault
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

        Ok(ctx)
    }
}

/// This function syncs a `KaminoIntegration`. This can be divided into two actions:
/// - If the kamino reserve associated with this integration has a `farm_collateral`,
///     and the corresponding remaining accounts are included, it harvests the rewards
///     through the `rewards_ata` account, created if needed.
///     If the `reward_mint` matches this integration mint, the corresponding accounting
///     events are emitted.
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
    let reserve_vault_balance_before = reserve.last_balance;

    // Get the kamino reserve state
    let kamino_reserve_data = inner_ctx.kamino_reserve.try_borrow_data()?;
    let kamino_reserve_state = KaminoReserve::try_from_slice(&kamino_reserve_data)?;

    let clock = Clock::get()?;

    // Check if the reserve is stale
    // Use NONE as this is not a borrow
    // if we want to borrow we should use the PriceStatusFlags::ALL_CHECKS
    // Note: we intentionally fail to ensure the accounting at the slot is correct
    // and the client must prepend the refresh IX to prevent error.
    if kamino_reserve_state
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg! {"reserve is stale"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Claim farm rewards only if the reserve has a farm collateral
    // and the remaining accounts are included.
    if kamino_reserve_state.has_collateral_farm() && inner_ctx.remaining_accounts.len() > 0 {
        // validate remaining accounts
        let harvest_ctx = HarvestKaminoAccounts::checked_from_accounts(
            inner_ctx.obligation.key(),
            inner_ctx.remaining_accounts,
        )?;

        // Validate that the reserve farm_collateral matches the reserve farm
        if kamino_reserve_state
            .farm_collateral
            .ne(harvest_ctx.kamino_reserve_farm.key())
        {
            msg! {"reserve_farm: Invalid address"}
            return Err(ProgramError::InvalidAccountData);
        }

        // Find the reward index in the FarmState of this kamino_reserve
        let (reward_index, rewards_available) = {
            let reserve_farm_data = harvest_ctx.kamino_reserve_farm.try_borrow_data()?;
            let reserve_farm_state = FarmState::try_from_slice(&reserve_farm_data)?;
            reserve_farm_state
                .find_reward_index_and_rewards_available(
                    harvest_ctx.rewards_mint.key(),
                    harvest_ctx.rewards_token_program.key(),
                )
                .ok_or(ProgramError::InvalidAccountData)?
        };

        // Only harvest rewards if rewards_available > 0
        if rewards_available > 0 {
            // Initialize ATA if needed
            CreateIdempotent {
                funding_account: outer_ctx.payer,
                account: harvest_ctx.rewards_ata,
                wallet: outer_ctx.controller_authority,
                mint: harvest_ctx.rewards_mint,
                system_program: harvest_ctx.system_program,
                token_program: harvest_ctx.rewards_token_program,
            }
            .invoke()?;

            // Claim farms rewards
            HarvestReward {
                owner: outer_ctx.controller_authority,
                user_state: harvest_ctx.obligation_farm,
                farm_state: harvest_ctx.kamino_reserve_farm,
                global_config: harvest_ctx.farms_global_config,
                reward_mint: harvest_ctx.rewards_mint,
                user_reward_ata: harvest_ctx.rewards_ata,
                rewards_vault: harvest_ctx.rewards_vault,
                rewards_treasury_vault: harvest_ctx.rewards_treasury_vault,
                farm_vaults_authority: harvest_ctx.farm_vaults_authority,
                scope_prices: harvest_ctx.scope_prices,
                token_program: harvest_ctx.rewards_token_program,
                reward_index,
            }
            .invoke_signed(&[Signer::from(&[
                Seed::from(CONTROLLER_AUTHORITY_SEED),
                Seed::from(outer_ctx.controller.key()),
                Seed::from(&[controller.authority_bump]),
            ])])?;

            // If there is a match between the reward_mint and the integration mint, emit event
            if harvest_ctx.rewards_mint.key().eq(&reserve.mint) {
                // Since the mints match, the reward_ata == reserve_vault
                let vault = TokenAccount::from_account_info(&inner_ctx.reserve_vault)?;
                let reserve_vault_balance_after = vault.amount();
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
                        mint: *harvest_ctx.rewards_mint.key(),
                        action: AccountingAction::Sync,
                        delta: reserve_vault_balance_delta,
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
                        mint: *harvest_ctx.rewards_mint.key(),
                        action: AccountingAction::Withdrawal,
                        // NOTE: we use the Reserve vault delta rather then the
                        // delta of the `rewards_vault`. This is because the kfarms
                        // program sends rewards to both the User and the Treasury.
                        // This is safe for accounting as Kamino does not allow tokens
                        // that have TransferFees > 0.
                        delta: reserve_vault_balance_delta,
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
                        mint: *harvest_ctx.rewards_mint.key(),
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
