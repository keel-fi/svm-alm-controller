use pinocchio::{
    account_info::AccountInfo, 
    instruction::{Seed, Signer}, msg, 
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar}
};

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED, 
    define_account_struct, 
    enums::{IntegrationConfig, IntegrationState}, 
    instructions::{InitializeArgs, InitializeIntegrationArgs}, 
    integrations::utilization_market::{
        config::UtilizationMarketConfig, kamino::{
            config::KaminoConfig, 
            cpi::{
                derive_market_authority_address, 
                derive_obligation_farm_address, 
                derive_user_metadata_address, 
                derive_vanilla_obligation_address, 
                initialize_obligation_cpi, 
                initialize_obligation_farm_for_reserve_cpi, 
                initialize_user_lookup_table, 
                initialize_user_metadata_cpi
            }, 
            kamino_state::{Obligation, Reserve}, state::KaminoState
        }, state::UtilizationMarketState, 
        KAMINO_FARMS_PROGRAM_ID, 
        KAMINO_LEND_PROGRAM_ID, 
        LOOKUP_TABLE_PROGRAM_ID
    }, 
    processor::InitializeIntegrationAccounts, 
    state::Controller
};

define_account_struct! {
    pub struct InitializeKaminoAccounts<'info> {
        obligation: mut;
        token_mint: @owner(pinocchio_token::ID);
        user_metadata: mut;
        user_lookup_table: mut;
        obligation_farm: mut;
        reserve: mut @owner(KAMINO_LEND_PROGRAM_ID);
        reserve_farm: mut; // TODO: verify if the farm state is the owner
        market_authority;
        market: @owner(KAMINO_LEND_PROGRAM_ID);
        lookup_table_program: @pubkey(LOOKUP_TABLE_PROGRAM_ID);
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

        // verify obligation pubkey is valid
        let obligation_pda = derive_vanilla_obligation_address(
            obligation_id,
            controller_authority.key(), 
            ctx.market.key(), 
            ctx.kamino_program.key()
        );
        if &obligation_pda != ctx.obligation.key() {
            msg! {"kamino obligation: Invalid address"}
            return Err(ProgramError::InvalidSeeds)
        }

        // verify metadata pubkey is valid
        let user_metadata_pda = derive_user_metadata_address(
            controller_authority.key(), 
            ctx.kamino_program.key()
        );
        if &user_metadata_pda != ctx.user_metadata.key() {
            msg! {"user metadata: Invalid address"}
            return Err(ProgramError::InvalidSeeds)
        }

        // verify obligation farm is valid
        let obligation_farm_pda = derive_obligation_farm_address(
            ctx.reserve_farm.key(), 
            ctx.obligation.key(), 
            ctx.kamino_farms_program.key()
        );
        if &obligation_farm_pda != ctx.obligation_farm.key() {
            msg! {"Obligation farm: Invalid address"}
            return Err(ProgramError::InvalidSeeds)
        }

        // verify market authority is valid
        let market_authority_pda = derive_market_authority_address(
            ctx.market.key(), 
            ctx.kamino_program.key()
        );
        if &market_authority_pda != ctx.market_authority.key() {
            msg! {"market authority: Invalid address"}
            return Err(ProgramError::InvalidSeeds)
        }

        Ok(ctx)
    }
}

// TODOs:
// 1- Verify that a vanilla obligation is what we actually want.
// 2- If we need support for referrer (optional account in initialize_user_metadata_cpi).
// 3- Verify the variant for created lookup table.
// 4- The mode for creating obligation farm (what is it for?).
// 5- which program owns the reserve farm (reserve.collateral_farm)? is it kamino_program of kamino_farms?



/// This function initializes a `KaminoIntegration`.
/// In order to do so it initializes (if needed):
/// - A `LUT` and `user_metadata_account` (initialized only once at the `controller` level).
/// - An `obligation` : an `obligation`s can be shared accross 8 different `KaminoIntegrations`, 
///     thats why we need an `obligation_id` arg. The `obligation` is derived from the `obligation_id`, 
///     the `market` and the `controller_authority`.
/// - An `obligation_farm`: derived from the `reserve.collateral_farm` and `obligation`, 
///     so every `KaminoIntegration` has its own `obligation_farm`.
pub fn process_initialize_kamino(
    controller: &Controller,
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_kamino");

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

    let reserve = Reserve::try_from(
        inner_ctx.reserve.try_borrow_data()?.as_ref()
    )?;
    reserve.check_from_account(&inner_ctx)?;

    // initialize LUT and metadata if owned by system program
    if inner_ctx.user_metadata.is_owned_by(&pinocchio_system::ID) {
        initialize_lut_and_metadata(
            outer_ctx, 
            &inner_ctx, 
            controller
        )?;
    }
    
    // initialize obligation if owned by system program
    if inner_ctx.obligation.is_owned_by(&pinocchio_system::ID) {
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
        obligation.check_from_accounts(outer_ctx, &inner_ctx)?;
    }

    // initialize obligation farm for the reserve we are targeting
    initialize_obligation_farm(outer_ctx, &inner_ctx)?;
    
    // create the config
    let kamino_config = KaminoConfig {
        market: *inner_ctx.market.key(),
        token_mint: *inner_ctx.token_mint.key(),
        obligation: *inner_ctx.obligation.key(),
        obligation_id
    };
    let config = IntegrationConfig::UtilizationMarket(
        UtilizationMarketConfig::KaminoConfig(kamino_config)
    );

    // create the state
    let kamino_state = KaminoState {
        assets: 0,
        liabilities: 0
    };
    let state = IntegrationState::UtilizationMarket(
        UtilizationMarketState::KaminoState(kamino_state)
    );

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
        0, // tag 0 for vanilla obligation
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
    outer_ctx: &InitializeIntegrationAccounts,
    inner_ctx: &InitializeKaminoAccounts,
) -> Result<(), ProgramError> {
    initialize_obligation_farm_for_reserve_cpi(
        outer_ctx.payer, 
        outer_ctx.controller_authority, 
        inner_ctx.obligation, 
        inner_ctx.market_authority, 
        inner_ctx.reserve, 
        inner_ctx.reserve_farm, 
        inner_ctx.obligation_farm, 
        inner_ctx.market, 
        inner_ctx.kamino_farms_program, 
        inner_ctx.rent, 
        inner_ctx.system_program, 
        inner_ctx.kamino_program.key()
    )?;

    Ok(())
}