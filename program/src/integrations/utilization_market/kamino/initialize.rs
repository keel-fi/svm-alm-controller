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
    instructions::InitializeIntegrationArgs, 
    integrations::utilization_market::{
        kamino::{cpi::{
            derive_market_authority_address, derive_obligation_address, derive_obligation_farm_address, derive_user_metadata_address, initialize_obligation_cpi, initialize_obligation_farm_for_reserve_cpi, initialize_user_lookup_table, initialize_user_metadata_cpi
        }, kamino_state::{Obligation, Reserve}}, 
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
        mint: @owner(pinocchio_token::ID);
        user_metadata: mut;
        user_lookup_table: mut;
        obligation_farm: mut;
        reserve: mut @owner(KAMINO_LEND_PROGRAM_ID);
        reserve_farm: mut;
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
        controller_authority: &'info AccountInfo
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;

        // verify obligation is not initialized
        if !ctx.obligation.is_owned_by(&pinocchio_system::ID) {
            msg! {"kamino obligation: not owned by system program"}
            return Err(ProgramError::InvalidAccountOwner)
        }

        let obligation_pda = derive_obligation_address(
            controller_authority.key(), 
            ctx.market.key(), 
            ctx.kamino_program.key()
        );

        // verify obligation pubkey is valid
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

/// Initializes a Kamino obligation,
/// One obligation per controller per market per reserve mint
pub fn process_initialize_kamino(
    controller: &Controller,
    outer_ctx: &InitializeIntegrationAccounts,
    _outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_kamino");

    let inner_ctx = 
        InitializeKaminoAccounts::checked_from_accounts(
            outer_ctx.remaining_accounts,
            outer_ctx.controller_authority
        )?;

    let reserve = Reserve::try_from(
        inner_ctx.reserve.try_borrow_data()?.as_ref()
    )?;

    // verify if reserve corresponds to market
    if &reserve.lending_market != inner_ctx.market.key() {
        msg! {"Reserve"}
        return Err(ProgramError::InvalidAccountData)
    }

    // verify liquidity_mint passed correspond to reserve.liquidity
    if &reserve.liquidity_mint != inner_ctx.mint.key() {
        return Err(ProgramError::InvalidAccountData)
    }

    if &reserve.farm_collateral != inner_ctx.reserve_farm.key() {
        return Err(ProgramError::InvalidAccountData)
    }

    // check if user metadata is initialized
    // one user metadata per controller! so only needs to be created once
    if inner_ctx.user_metadata.is_owned_by(&pinocchio_system::ID) {
        // if not initialized, create LUT and user metadata
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
    }    
    
    // initialize the obligation only if it doesnt exist already
    if inner_ctx.obligation.is_owned_by(&pinocchio_system::ID) {
        initialize_obligation_cpi(
            0, 
            0, 
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
    } else {
        // validate obligation is OK
        let obligation = Obligation::try_from(
            inner_ctx.obligation.try_borrow_data()?.as_ref()
        )?;

        if &obligation.lending_market != inner_ctx.market.key() {

        }

        if &obligation.owner != outer_ctx.controller_authority.key() {

        }

        // make sure this is actually needed! 
        if !obligation.collateral_reserves.contains(inner_ctx.reserve.key()) {

        }
    }

    // initialize obligation farm for the reserve we are targeting
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
    
    todo!()
}