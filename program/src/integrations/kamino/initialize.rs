use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::{
        kamino::{
            config::KaminoConfig,
            constants::{
                KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID, OBLIGATION_FARM_COLLATERAL_MODE,
                VANILLA_OBLIGATION_TAG,
            },
            cpi::{
                InitializeObligation, InitializeObligationFarmForReserve, InitializeUserMetadata,
            },
            klend_protocol_state::{KaminoReserve, Obligation},
            pdas::{
                derive_market_authority_address, derive_obligation_farm_address,
                derive_user_metadata_address, derive_vanilla_obligation_address,
            },
        },
        shared::lending_markets::LendingState,
    },
    processor::{shared::validate_mint_extensions, InitializeIntegrationAccounts},
    state::Controller,
};

define_account_struct! {
    pub struct InitializeKaminoAccounts<'info> {
        // May or may not be owned by Kamino program. Check performed
        // in processor.
        obligation: mut;
        reserve_liquidity_mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        // May or may not be owned by Kamino program. Check performed
        // in processor.
        user_metadata: mut;
        referrer_metadata;
        obligation_farm_collateral: mut @owner(KAMINO_FARMS_PROGRAM_ID, pinocchio_system::ID);
        kamino_reserve: mut @owner(KAMINO_LEND_PROGRAM_ID);
        reserve_farm_collateral: mut;
        market_authority;
        market: @owner(KAMINO_LEND_PROGRAM_ID);
        kamino_lend_program: @pubkey(KAMINO_LEND_PROGRAM_ID);
        kamino_farms_program: @pubkey(KAMINO_FARMS_PROGRAM_ID);
        system_program: @pubkey(pinocchio_system::ID);
        rent: @pubkey(pinocchio::sysvars::rent::RENT_ID);
    }
}

impl<'info> InitializeKaminoAccounts<'info> {
    pub fn checked_from_accounts(
        account_infos: &'info [AccountInfo],
        controller_authority: &'info AccountInfo,
        obligation_id: u8,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;

        // Ensure the mint has valid T22 extensions.
        validate_mint_extensions(ctx.reserve_liquidity_mint, &[])?;

        // reserve.farm_collateral can either be pubkey::default or be owned by kamino_farms program
        if ctx.reserve_farm_collateral.key().ne(&Pubkey::default())
            && !ctx
                .reserve_farm_collateral
                .is_owned_by(ctx.kamino_farms_program.key())
        {
            msg! {"reserve_farm_collateral: Invalid owner"}
            return Err(ProgramError::InvalidAccountOwner);
        }

        // verify obligation pubkey is valid
        let obligation_pda = derive_vanilla_obligation_address(
            obligation_id,
            controller_authority.key(),
            ctx.market.key(),
            ctx.kamino_lend_program.key(),
        )?;
        if obligation_pda.ne(ctx.obligation.key()) {
            msg! {"kamino obligation: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // verify metadata pubkey is valid
        let user_metadata_pda =
            derive_user_metadata_address(controller_authority.key(), ctx.kamino_lend_program.key())?;
        if user_metadata_pda.ne(ctx.user_metadata.key()) {
            msg! {"user metadata: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // verify obligation farm collateral is valid
        let obligation_farm_collateral_pda = derive_obligation_farm_address(
            ctx.reserve_farm_collateral.key(),
            ctx.obligation.key(),
            ctx.kamino_farms_program.key(),
        )?;
        if obligation_farm_collateral_pda.ne(ctx.obligation_farm_collateral.key()) {
            msg! {"Obligation farm collateral: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // verify market authority is valid
        let market_authority_pda =
            derive_market_authority_address(ctx.market.key(), ctx.kamino_lend_program.key())?;
        if market_authority_pda.ne(ctx.market_authority.key()) {
            msg! {"market authority: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        // referrer_metadata can be either pubkey == KLEND (None variant of Optional)
        // or be owned by KLEND.
        if ctx.referrer_metadata.key().ne(ctx.kamino_lend_program.key())
            && !ctx.referrer_metadata.is_owned_by(ctx.kamino_lend_program.key())
        {
            msg! {"referrer_metadata: Invalid owner"}
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(ctx)
    }
}

/// This function initializes a `KaminoIntegration`.
/// In order to do so it initializes (if needed):
/// - A `user_metadata_account` (initialized only once at the `controller` level).
/// - An `obligation` : The `obligation` is derived from the `obligation_id`,
///     the `market` and the `controller_authority`. An `obligation` can be shared across many `KaminoIntegration`s,
///     but up to 8 can be active (see field `ObligationCollateral`).
/// - An `obligation_farm`: derived from the `reserve.collateral_farm` and `obligation`,
///     so every `KaminoIntegration` has its own `obligation_farm` IF the reserve has a collateral_farm.
///
/// **Important**: This instruction initializes by default a "Vanilla" kamino Obligation.
pub fn process_initialize_kamino(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
    controller: &Controller,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_kamino");

    let obligation_id = match outer_args.inner_args {
        InitializeArgs::KaminoIntegration { obligation_id } => obligation_id,
        _ => return Err(ProgramError::InvalidArgument),
    };

    let inner_ctx = InitializeKaminoAccounts::checked_from_accounts(
        outer_ctx.remaining_accounts,
        outer_ctx.controller_authority,
        obligation_id,
    )?;

    let kamino_reserve_has_collateral_farm = {
        let kamino_reserve_data = inner_ctx.kamino_reserve.try_borrow_data()?;
        let kamino_reserve = KaminoReserve::try_from_slice(&kamino_reserve_data)?;
        kamino_reserve.check_from_init_accounts(&inner_ctx)?;
        kamino_reserve.has_collateral_farm()
    };

    // Initialize user metadata if owned by system program
    if inner_ctx.user_metadata.is_owned_by(&pinocchio_system::ID) {
        InitializeUserMetadata {
            owner: outer_ctx.controller_authority,
            payer: outer_ctx.payer,
            user_metadata: inner_ctx.user_metadata,
            referrer_user_metadata: inner_ctx.referrer_metadata,
            rent: inner_ctx.rent,
            system_program: inner_ctx.system_program,
            user_lookup_table: Pubkey::default(),
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ])])?;
    }

    // Initialize obligation if owned by system program
    if inner_ctx.obligation.is_owned_by(&pinocchio_system::ID) {
        InitializeObligation {
            obligation_owner: outer_ctx.controller_authority,
            payer: outer_ctx.payer,
            obligation: inner_ctx.obligation,
            lending_market: inner_ctx.market,
            // System program AccountInfo is used since
            // seed 1 and seed 2 are default values
            // for VanillaObligations
            seed_1: inner_ctx.system_program,
            seed_2: inner_ctx.system_program,
            owner_user_metadata: inner_ctx.user_metadata,
            rent: inner_ctx.rent,
            system_program: inner_ctx.system_program,
            tag: VANILLA_OBLIGATION_TAG,
            id: obligation_id,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump]),
        ])])?;
    } else {
        // Validate obligation is OK
        let obligation_data = inner_ctx.obligation.try_borrow_data()?;
        let obligation = Obligation::try_from_slice(&obligation_data)?;

        obligation.check_data(outer_ctx.controller_authority.key(), inner_ctx.market.key())?;
    }

    // Initialize obligation farm for the reserve we are targeting,
    // only if the reserve has a collateral_farm
    // and the account is owned by system program
    if kamino_reserve_has_collateral_farm
        && inner_ctx
            .obligation_farm_collateral
            .is_owned_by(&pinocchio_system::ID)
    {
        InitializeObligationFarmForReserve {
            payer: outer_ctx.payer,
            owner: outer_ctx.controller_authority,
            obligation: inner_ctx.obligation,
            market_authority: inner_ctx.market_authority,
            kamino_reserve: inner_ctx.kamino_reserve,
            reserve_farm_state: inner_ctx.reserve_farm_collateral,
            obligation_farm: inner_ctx.obligation_farm_collateral,
            lending_market: inner_ctx.market,
            farms_program: inner_ctx.kamino_farms_program,
            rent: inner_ctx.rent,
            system_program: inner_ctx.system_program,
            mode: OBLIGATION_FARM_COLLATERAL_MODE,
        }
        .invoke()?;
    }

    // Create the config
    let kamino_config = KaminoConfig {
        market: *inner_ctx.market.key(),
        reserve: *inner_ctx.kamino_reserve.key(),
        reserve_liquidity_mint: *inner_ctx.reserve_liquidity_mint.key(),
        obligation: *inner_ctx.obligation.key(),
        obligation_id,
        _padding: [0; 95],
    };
    let config = IntegrationConfig::Kamino(kamino_config);

    // Create the state
    let state = IntegrationState::Kamino(LendingState {
        balance: 0,
        _padding: [0u8; 40],
    });

    Ok((config, state))
}
