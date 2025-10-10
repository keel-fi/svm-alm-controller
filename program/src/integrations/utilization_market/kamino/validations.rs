use crate::{
    define_account_struct, 
    enums::IntegrationConfig, 
    error::SvmAlmControllerErrors, 
    integrations::utilization_market::{
        config::UtilizationMarketConfig, 
        kamino::{
            constants::{KAMINO_FARMS_PROGRAM_ID,KAMINO_LEND_PROGRAM_ID}, 
            cpi::{
                derive_market_authority_address, 
                derive_reserve_collateral_mint, 
                derive_reserve_collateral_supply, 
                derive_reserve_liquidity_supply
            }
        }
    }, state::Reserve
};
use pinocchio::{
    account_info::AccountInfo, 
    msg, 
    program_error::ProgramError, 
    pubkey::Pubkey, 
    sysvars::instructions::INSTRUCTIONS_ID
};
use pinocchio_token::state::TokenAccount;

define_account_struct! {
    pub struct PushPullKaminoAccounts<'info> {
        // Pull = liquidity_destination, Push = liquidity_source
        token_account: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        obligation: mut @owner(KAMINO_LEND_PROGRAM_ID);
        kamino_reserve: mut @owner(KAMINO_LEND_PROGRAM_ID);
        reserve_liquidity_mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_liquidity_supply: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_collateral_mint: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_collateral_supply: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        market_authority;
        market: @owner(KAMINO_LEND_PROGRAM_ID);
        collateral_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        liquidity_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        instruction_sysvar_account: @pubkey(INSTRUCTIONS_ID);
        obligation_farm_collateral: mut;
        reserve_farm_collateral: mut;
        kamino_farms_program: @pubkey(KAMINO_FARMS_PROGRAM_ID);
        kamino_program: @pubkey(KAMINO_LEND_PROGRAM_ID);
    }
}

impl<'info> PushPullKaminoAccounts<'info> {
    /// Builds `PushPullKaminoAccounts` and validates identities:
/// - Config (Kamino): market, reserve, reserve_farm_collateral, reserve_liquidity_mint, obligation
/// - KLend PDAs: reserve_{collateral_mint, collateral_supply, liquidity_supply}, market_authority
/// - token_account: mint == reserve_liquidity_mint, owner == controller_authority, key == reserve.vault
/// - reserve.mint == reserve_liquidity_mint
/// Returns ctx or `InvalidAccountData`/`InvalidPda`. Use for both push and pull.
    pub fn checked_from_accounts(
        controller_authority: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
        reserve: &Reserve
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::UtilizationMarket(c) => {
                match c {
                    UtilizationMarketConfig::KaminoConfig(kamino_config) => kamino_config,
                }
            },
            _ => return Err(ProgramError::InvalidAccountData),
        };

        if ctx.market.key().ne(&config.market) {
            msg! {"market: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.kamino_reserve.key().ne(&config.reserve) {
            msg! {"kamino_reserve: does not match config"};
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

        let reserve_collateral_mint_pda = derive_reserve_collateral_mint(
            &ctx.market.key(), 
            &ctx.reserve_liquidity_mint.key(), 
            &KAMINO_LEND_PROGRAM_ID
        )?;
        if ctx.reserve_collateral_mint.key().ne(&reserve_collateral_mint_pda) {
            msg! {"reserve_collateral_mint: does not match PDA"};
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let reserve_collateral_supply_pda = derive_reserve_collateral_supply(
            &ctx.market.key(), 
            &ctx.reserve_liquidity_mint.key(), 
            &KAMINO_LEND_PROGRAM_ID
        )?;
        if ctx.reserve_collateral_supply.key().ne(&reserve_collateral_supply_pda) {
            msg! {"reserve_collateral_supply: does not match PDA"};
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let reserve_liquidity_supply_pda = derive_reserve_liquidity_supply(
            &ctx.market.key(), 
            &ctx.reserve_liquidity_mint.key(), 
            &KAMINO_LEND_PROGRAM_ID
        )?;
        if ctx.reserve_liquidity_supply.key().ne(&reserve_liquidity_supply_pda) {
            msg! {"reserve_liquidity_supply: does not match PDA"};
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        if ctx.obligation.key().ne(&config.obligation) {
            msg! {"obligation: does not match config"};
            return Err(ProgramError::InvalidAccountData);
        }

        let market_authority_pda = derive_market_authority_address(
            ctx.market.key(), 
            &KAMINO_LEND_PROGRAM_ID
        )?;
        if ctx.market_authority.key().ne(&market_authority_pda)  {
            msg! {"market authority: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into())
        }

        let token_account 
            = TokenAccount::from_account_info(ctx.token_account)?;
        if token_account.mint().ne(&config.reserve_liquidity_mint) {
            msg! {"token_account_info: invalid mint"};
            return Err(ProgramError::InvalidAccountData);
        }
        if token_account.owner().ne(controller_authority) {
            msg! {"token_account_info: not owned by Controller authority PDA"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.token_account.key().ne(&reserve.vault) {
            msg! {"token_account_info: mismatch with reserve"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.reserve_liquidity_mint.key().ne(&reserve.mint) {
            msg! {"reserve_liquidity_mint: mismatch with reserve"};
            return Err(ProgramError::InvalidAccountData)
        }

        Ok(ctx)
    }
}


