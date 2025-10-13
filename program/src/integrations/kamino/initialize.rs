use borsh::maybestd::format;
use pinocchio::{
    account_info::AccountInfo, instruction::{Seed, Signer}, 
    msg, program_error::ProgramError, 
    pubkey::Pubkey, sysvars::{clock::Clock, Sysvar}
};

use crate::{
    constants::{ADDRESS_LOOKUP_TABLE_PROGRAM_ID, CONTROLLER_AUTHORITY_SEED}, 
    define_account_struct, 
    enums::{IntegrationConfig, IntegrationState}, 
    error::SvmAlmControllerErrors, 
    instructions::{InitializeArgs, InitializeIntegrationArgs}, 
    integrations::kamino::{
        config::KaminoConfig, 
        constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID}, 
        cpi::{
            derive_market_authority_address, 
            derive_obligation_farm_address, 
            derive_user_metadata_address, 
            derive_vanilla_obligation_address, 
            initialize_obligation_cpi, 
            initialize_obligation_farm_for_reserve_cpi, 
            initialize_user_lookup_table, 
            initialize_user_metadata_cpi, 
            OBLIGATION_FARM_COLLATERAL_MODE,
        }, 
        kamino_state::{KaminoReserve, Obligation}, 
        state::KaminoState
    }, 
    processor::InitializeIntegrationAccounts, 
    state::Controller
};

define_account_struct! {
    pub struct InitializeKaminoAccounts<'info> {
        obligation: mut;
        reserve_liquidity_mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        user_metadata: mut;
        user_lookup_table: mut;
        referrer_metadata;
        obligation_farm_collateral: mut @owner(KAMINO_FARMS_PROGRAM_ID, pinocchio_system::ID);
        obligation_farm_debt: mut @owner(KAMINO_FARMS_PROGRAM_ID, pinocchio_system::ID);
        kamino_reserve: mut @owner(KAMINO_LEND_PROGRAM_ID);
        reserve_farm_collateral: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        reserve_farm_debt: mut;
        market_authority;
        market: @owner(KAMINO_LEND_PROGRAM_ID);
        lookup_table_program: @pubkey(ADDRESS_LOOKUP_TABLE_PROGRAM_ID);
        kamino_program: @pubkey(KAMINO_LEND_PROGRAM_ID);
        kamino_farms_program: @pubkey(KAMINO_FARMS_PROGRAM_ID);
        system_program: @pubkey(pinocchio_system::ID);
        rent: @pubkey(pinocchio::sysvars::rent::RENT_ID);
    }
}

impl<'info> InitializeKaminoAccounts<'info> {
    pub fn checked_from_accounts(
        account_infos: &'info [AccountInfo],
        controller_authority: &'info AccountInfo,
        obligation_id: u8
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;

        // reserve.farm_debt can either be pubkey::default or owned by kamino_farms program
        if ctx.reserve_farm_debt.key().ne(&Pubkey::default())
            && !ctx.reserve_farm_collateral.is_owned_by(&KAMINO_FARMS_PROGRAM_ID) 
        {
            msg! {"reserve_farm_collateral: Invalid owner"}
            return Err(ProgramError::IllegalOwner)
        }

        // verify obligation pubkey is valid
        let obligation_pda = derive_vanilla_obligation_address(
            obligation_id,
            controller_authority.key(), 
            ctx.market.key(), 
            ctx.kamino_program.key()
        )?;
        if obligation_pda.ne(ctx.obligation.key()) {
            msg! {"kamino obligation: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // verify metadata pubkey is valid
        let user_metadata_pda = derive_user_metadata_address(
            controller_authority.key(), 
            ctx.kamino_program.key()
        )?;
        if user_metadata_pda.ne(ctx.user_metadata.key()) {
            msg! {"user metadata: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // verify obligation farm collateral is valid 
        let obligation_farm_collateral_pda = derive_obligation_farm_address(
            ctx.reserve_farm_collateral.key(), 
            ctx.obligation.key(), 
            ctx.kamino_farms_program.key()
        )?;
        if obligation_farm_collateral_pda.ne(ctx.obligation_farm_collateral.key()) {
            msg! {"Obligation farm collateral: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // verify obligation farm debt is valid
        // NOTE: This is not required for depositing (klend-sdk doesnt use it), but maybe in the future for borrowing?)
        let obligation_farm_debt_pda = derive_obligation_farm_address(
            ctx.reserve_farm_debt.key(), 
            ctx.obligation.key(), 
            ctx.kamino_farms_program.key()
        )?;
        if obligation_farm_debt_pda.ne(ctx.obligation_farm_debt.key()) {
            msg! {"Obligation farm collateral: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        // verify market authority is valid
        let market_authority_pda = derive_market_authority_address(
            ctx.market.key(), 
            ctx.kamino_program.key()
        )?;
        if market_authority_pda.ne(ctx.market_authority.key()) {
            msg! {"market authority: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        Ok(ctx)
    }
}

/// This function initializes a `KaminoIntegration`.
/// In order to do so it initializes (if needed):
/// - A `LUT` and `user_metadata_account` (initialized only once at the `controller` level).
/// - An `obligation` : The `obligation` is derived from the `obligation_id`, 
///     the `market` and the `controller_authority`. An `obligation` can be shared accross many `KaminoIntegration`s,
///     but up to 8 can be active (see field ObligationCollateral).
/// - An `obligation_farm`: derived from the `reserve.collateral_farm` and `obligation`, 
///     so every `KaminoIntegration` has its own `obligation_farm` IF the reserve has a collateral_farm.
/// 
/// **Important**: This instruction initializes by default a "Vanilla" kamino Obligation, since that's
/// what's used in the `klend-sdk` examples. Also, `obligation_farm_debt` is not supported at the moment.
pub fn process_initialize_kamino(
    controller: &Controller,
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_kamino");

    msg!(&format!("inner_args: {:?}", outer_args.inner_args));
    let obligation_id = match outer_args.inner_args {
        InitializeArgs::KaminoIntegration { 
            obligation_id 
        } => obligation_id,
        _ => return Err(ProgramError::InvalidArgument),
    };

    let inner_ctx = 
        InitializeKaminoAccounts::checked_from_accounts(
            outer_ctx.remaining_accounts,
            outer_ctx.controller_authority,
            obligation_id
        )?;

    let kamino_reserve = KaminoReserve::try_from(
        inner_ctx.kamino_reserve.try_borrow_data()?.as_ref()
    )?;
    kamino_reserve.check_from_init_accounts(&inner_ctx)?;

    // initialize an address lookup table and metadata if owned by system program
    if inner_ctx.user_metadata.is_owned_by(&pinocchio_system::ID) {
        msg! {"calling initialize_lut_and_metadata"}
        initialize_lut_and_metadata(
            outer_ctx, 
            &inner_ctx, 
            controller
        )?;
    }
    
    // initialize obligation if owned by system program
    if inner_ctx.obligation.is_owned_by(&pinocchio_system::ID) {
        msg! {"calling initialize_vanilla_obligation"}
        initialize_vanilla_obligation(
            obligation_id, 
            outer_ctx, 
            &inner_ctx, 
            controller
        )?;
    } else {
        // validate obligation is OK
        let obligation = Obligation::try_from(
            inner_ctx.obligation.try_borrow_data()?.as_ref()
        )?;
        obligation.check_data(
            outer_ctx.controller_authority.key(), 
            inner_ctx.market.key()
        )?;
    }

    // initialize obligation farm for the reserve we are targeting,
    // only if the reserve has a collateral_farm
    if kamino_reserve.has_collateral_farm() {
        initialize_obligation_farm(OBLIGATION_FARM_COLLATERAL_MODE, outer_ctx, &inner_ctx)?;
    }

    // NOTE: removed this until it is required / better understood
    // initialize an obligation farm, only if reserve has farm_debt
    // if reserve.has_debt_farm() {
    //     initialize_obligation_farm(OBLIGATION_FARM_DEBT_MODE, outer_ctx, &inner_ctx)?;
    // }
    
    // create the config
    let kamino_config = KaminoConfig {
        market: *inner_ctx.market.key(),
        reserve: *inner_ctx.kamino_reserve.key(),
        reserve_farm_collateral: *inner_ctx.reserve_farm_collateral.key(),
        reserve_farm_debt: *inner_ctx.reserve_farm_debt.key(),
        reserve_liquidity_mint: *inner_ctx.reserve_liquidity_mint.key(),
        obligation: *inner_ctx.obligation.key(),
        obligation_id,
        _padding: [0; 31]
    };
    let config = IntegrationConfig::Kamino(kamino_config);

    // create the state
    let kamino_state = KaminoState {
        last_liquidity_value: 0,
        last_lp_amount: 0,
        _padding: [0; 32]
    };
    let state = IntegrationState::Kamino(kamino_state);

    Ok((config, state))
}

fn initialize_lut_and_metadata(
    outer_ctx: &InitializeIntegrationAccounts,
    inner_ctx: &InitializeKaminoAccounts,
    controller: &Controller
) -> Result<(), ProgramError> {
    
    initialize_user_lookup_table(
        Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump])
        ]), 
        outer_ctx.controller_authority, 
        outer_ctx.payer, 
        inner_ctx.user_lookup_table, 
        inner_ctx.lookup_table_program.key(), 
        inner_ctx.system_program, 
        Clock::get()?.slot
    )?;

    initialize_user_metadata_cpi(
        Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump])
        ]), 
        outer_ctx.controller_authority, 
        outer_ctx.payer, 
        inner_ctx.user_metadata, 
        inner_ctx.user_lookup_table, 
        inner_ctx.referrer_metadata,
        inner_ctx.kamino_program.key(), 
        inner_ctx.rent, 
        inner_ctx.system_program
    )?;

    Ok(())
}

fn initialize_vanilla_obligation(
    id: u8,
    outer_ctx: &InitializeIntegrationAccounts,
    inner_ctx: &InitializeKaminoAccounts,
    controller: &Controller
) -> Result<(), ProgramError> {
    initialize_obligation_cpi(
        id, 
        Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(outer_ctx.controller.key()),
            Seed::from(&[controller.authority_bump])
        ]), 
        inner_ctx.obligation, 
        outer_ctx.controller_authority, 
        outer_ctx.payer, 
        inner_ctx.market, 
        inner_ctx.user_metadata, 
        inner_ctx.kamino_program.key(), 
        inner_ctx.rent, 
        inner_ctx.system_program
    )?;

    Ok(())
}

fn initialize_obligation_farm(
    mode: u8,
    outer_ctx: &InitializeIntegrationAccounts,
    inner_ctx: &InitializeKaminoAccounts,
) -> Result<(), ProgramError> {

    let (reserve_farm, obligation_farm) = match mode {
        0 => {
            (inner_ctx.reserve_farm_collateral, inner_ctx.obligation_farm_collateral)
        },
        1 => {
            (inner_ctx.reserve_farm_debt, inner_ctx.obligation_farm_debt)
        },
        _ => return Err(ProgramError::InvalidArgument)
    };

    initialize_obligation_farm_for_reserve_cpi(
        mode,
        outer_ctx.payer, 
        outer_ctx.controller_authority, 
        inner_ctx.obligation, 
        inner_ctx.market_authority, 
        inner_ctx.kamino_reserve, 
        reserve_farm, 
        obligation_farm, 
        inner_ctx.market, 
        inner_ctx.kamino_farms_program, 
        inner_ctx.rent, 
        inner_ctx.system_program, 
        inner_ctx.kamino_program.key()
    )?;

    Ok(())
}