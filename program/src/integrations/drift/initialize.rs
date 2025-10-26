use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::account_info::AccountInfo;
use pinocchio::instruction::Seed;
use pinocchio::msg;
use pinocchio::sysvars::rent::RENT_ID;
use pinocchio::{instruction::Signer, program_error::ProgramError};

use crate::constants::CONTROLLER_AUTHORITY_SEED;
use crate::error::SvmAlmControllerErrors;
use crate::instructions::InitializeArgs;
use crate::integrations::drift::cpi::{InitializeUser, UpdateUserPoolId};
use crate::integrations::drift::pdas::{
    derive_drift_spot_market_pda, derive_drift_state_pda, derive_drift_user_pda,
    derive_drift_user_stats_pda,
};
use crate::integrations::drift::protocol_state::{SpotMarket, User};
use crate::integrations::shared::lending_markets::LendingState;
use crate::processor::shared::validate_mint_extensions;
use crate::state::Controller;
use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::InitializeIntegrationArgs,
    integrations::drift::{
        config::DriftConfig, constants::DRIFT_PROGRAM_ID, cpi::InitializeUserStats,
    },
    processor::InitializeIntegrationAccounts,
};

/*
Initialize a Drift Integration. Each integration is for a specific
Mint under a specific subaccount (aka User).
 */
define_account_struct! {
    pub struct InitializeDriftAccounts<'info> {
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        drift_user: mut;
        drift_user_stats: mut;
        drift_state: mut, @owner(DRIFT_PROGRAM_ID);
        drift_spot_market: @owner(DRIFT_PROGRAM_ID);
        rent: @pubkey(RENT_ID);
        drift_program: @pubkey(DRIFT_PROGRAM_ID);
    }
}

impl<'info> InitializeDriftAccounts<'info> {
    pub fn checked_from_accounts(
        account_infos: &'info [AccountInfo],
        controller_authority: &'info AccountInfo,
        sub_account_id: u16,
        spot_market_index: u16,
    ) -> Result<Self, ProgramError> {
        let ctx = InitializeDriftAccounts::from_accounts(account_infos)?;

        // Ensure the mint has valid T22 extensions.
        validate_mint_extensions(ctx.mint, &[])?;

        let drift_user_pda = derive_drift_user_pda(controller_authority.key(), sub_account_id)?;
        if drift_user_pda.ne(ctx.drift_user.key()) {
            msg! {"drift user: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let drift_user_stats_pda = derive_drift_user_stats_pda(controller_authority.key())?;
        if drift_user_stats_pda.ne(ctx.drift_user_stats.key()) {
            msg! {"drift user stats: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let drift_state_pda = derive_drift_state_pda()?;
        if drift_state_pda.ne(ctx.drift_state.key()) {
            msg! {"drift state: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let drift_spot_market_pda = derive_drift_spot_market_pda(spot_market_index)?;
        if drift_spot_market_pda.ne(ctx.drift_spot_market.key()) {
            msg! {"drift spot market: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        Ok(ctx)
    }
}

pub fn process_initialize_drift(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
    controller: &Controller,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    let (sub_account_id, spot_market_index) = match outer_args.inner_args {
        InitializeArgs::Drift {
            sub_account_id,
            spot_market_index,
        } => (sub_account_id, spot_market_index),
        _ => return Err(ProgramError::InvalidArgument),
    };

    let inner_ctx = InitializeDriftAccounts::checked_from_accounts(
        outer_ctx.remaining_accounts,
        outer_ctx.controller_authority,
        sub_account_id,
        spot_market_index,
    )?;

    // Check that the spot_market_index is valid and matches a Drift SpotMarket
    // and that the pool_id matches the user.pool_id (if already initialized), or
    // update the user pool_id (CPI) (if being initialized)
    let spot_market_data = inner_ctx.drift_spot_market.try_borrow_data()?;
    let spot_market = SpotMarket::try_from_slice(&spot_market_data)?;
    if spot_market.market_index != spot_market_index {
        msg!("spot_market: Invalid market index");
        return Err(ProgramError::InvalidAccountData);
    }
    if spot_market.mint.ne(inner_ctx.mint.key()) {
        msg!("spot_market: mint does not match");
        return Err(ProgramError::InvalidAccountData);
    }

    // Initialize UserStats if owned by system program
    if inner_ctx
        .drift_user_stats
        .is_owned_by(&pinocchio_system::ID)
    {
        InitializeUserStats {
            user_stats: inner_ctx.drift_user_stats,
            state: inner_ctx.drift_state,
            authority: outer_ctx.controller_authority,
            payer: outer_ctx.payer,
            rent: inner_ctx.rent,
            system_program: outer_ctx.system_program,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ])])?;
    }

    // Initialize Drift User if owned by system program
    if inner_ctx.drift_user.is_owned_by(&pinocchio_system::ID) {
        InitializeUser {
            user: inner_ctx.drift_user,
            user_stats: inner_ctx.drift_user_stats,
            state: inner_ctx.drift_state,
            authority: outer_ctx.controller_authority,
            payer: outer_ctx.payer,
            rent: inner_ctx.rent,
            system_program: outer_ctx.system_program,
            sub_account_id,
            // Name not important since these accounts will
            // not typically be read by a human. Instead, the
            // subaccount ID will be the canonical identifier.
            name: [0u8; 32],
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ])])?;

        // if pool_id is not 0, we update the Drift User account
        if spot_market.pool_id != 0 {
            UpdateUserPoolId {
                user: inner_ctx.drift_user,
                authority: outer_ctx.controller_authority,
                sub_account_id,
                pool_id: spot_market.pool_id
            }
            .invoke_signed(&[Signer::from(&[
                Seed::from(CONTROLLER_AUTHORITY_SEED),
                Seed::from(outer_ctx.controller.key()),
                Seed::from(&[controller.authority_bump]),
            ])])?;
        }
    } else {
        // Validate User.pool_id == SpotMarket.pool_id
        let user_data = inner_ctx.drift_user.try_borrow_data()?;
        let user = User::try_from_slice(&user_data)?;

        if user.pool_id != spot_market.pool_id {
            msg!("user: pool_id does not match spot_market pool id");
            return Err(ProgramError::InvalidAccountData);
        }
    }

    let config = IntegrationConfig::Drift(DriftConfig {
        sub_account_id,
        spot_market_index,
        pool_id: spot_market.pool_id,
        _padding: [0u8; 219],
    });
    let state = IntegrationState::Drift(LendingState {
        balance: 0,
        _padding: [0u8; 40],
    });

    Ok((config, state))
}
