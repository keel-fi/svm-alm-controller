use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    error::SvmAlmControllerErrors,
    integrations::kamino::{
        constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID},
        pdas::{
            derive_market_authority_address, derive_obligation_farm_address,
            derive_reserve_collateral_mint, derive_reserve_collateral_supply,
            derive_reserve_liquidity_supply,
        },
    },
    state::Reserve,
};
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey,
    sysvars::instructions::INSTRUCTIONS_ID,
};
use pinocchio_token_interface::TokenAccount;

define_account_struct! {
    pub struct PushPullKaminoAccounts<'info> {
        // Pull = liquidity_destination, Push = liquidity_source
        reserve_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        obligation: mut @owner(KAMINO_LEND_PROGRAM_ID);
        kamino_reserve: mut @owner(KAMINO_LEND_PROGRAM_ID);
        kamino_reserve_liquidity_mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        kamino_reserve_liquidity_supply: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        kamino_reserve_collateral_mint: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        kamino_reserve_collateral_supply: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        market_authority;
        market: @owner(KAMINO_LEND_PROGRAM_ID);
        // KLEND only supports spl token program for collateral_token_program
        // See: https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/handlers/handler_init_reserve.rs#L144
        collateral_token_program: @pubkey(pinocchio_token::ID);
        liquidity_token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        instruction_sysvar_account: @pubkey(INSTRUCTIONS_ID);
        obligation_farm_collateral: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        reserve_farm_collateral: mut @owner(KAMINO_FARMS_PROGRAM_ID);
        kamino_farms_program: @pubkey(KAMINO_FARMS_PROGRAM_ID);
        kamino_program: @pubkey(KAMINO_LEND_PROGRAM_ID);
    }
}

impl<'info> PushPullKaminoAccounts<'info> {
    /// Builds `PushPullKaminoAccounts` and validates identities:
    /// - Config (Kamino): market, kamino_reserve, kamino_reserve_liquidity_mint, obligation
    /// - KLend PDAs: kamino_reserve_{collateral_mint, collateral_supply, liquidity_supply}, market_authority
    /// - reserve_vault: mint == reserve_liquidity_mint, owner == controller_authority, key == reserve.vault
    /// - reserve.mint == reserve_liquidity_mint
    /// - obligation_farm_collateral: matches PDA derived from reserve_farm_collateral and obligation
    /// Returns ctx or `InvalidAccountData`/`InvalidPda`. Use for both push and pull.
    pub fn checked_from_accounts(
        controller_authority: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
        reserve: &Reserve,
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::Kamino(kamino_config) => kamino_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        // check_accounts verifies that the following pubkeys
        // match those stored in this integration config
        config.check_accounts(
            ctx.obligation.key(),
            ctx.kamino_reserve.key(),
            ctx.kamino_reserve_liquidity_mint.key(),
            Some(ctx.market.key()),
        )?;

        let reserve_collateral_mint_pda = derive_reserve_collateral_mint(
            &ctx.market.key(),
            &ctx.kamino_reserve_liquidity_mint.key(),
            &KAMINO_LEND_PROGRAM_ID,
        )?;
        if ctx
            .kamino_reserve_collateral_mint
            .key()
            .ne(&reserve_collateral_mint_pda)
        {
            msg! {"reserve_collateral_mint: does not match PDA"};
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let reserve_collateral_supply_pda = derive_reserve_collateral_supply(
            &ctx.market.key(),
            &ctx.kamino_reserve_liquidity_mint.key(),
            &KAMINO_LEND_PROGRAM_ID,
        )?;
        if ctx
            .kamino_reserve_collateral_supply
            .key()
            .ne(&reserve_collateral_supply_pda)
        {
            msg! {"reserve_collateral_supply: does not match PDA"};
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let reserve_liquidity_supply_pda = derive_reserve_liquidity_supply(
            &ctx.market.key(),
            &ctx.kamino_reserve_liquidity_mint.key(),
            &KAMINO_LEND_PROGRAM_ID,
        )?;
        if ctx
            .kamino_reserve_liquidity_supply
            .key()
            .ne(&reserve_liquidity_supply_pda)
        {
            msg! {"reserve_liquidity_supply: does not match PDA"};
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let market_authority_pda =
            derive_market_authority_address(ctx.market.key(), &KAMINO_LEND_PROGRAM_ID)?;
        if ctx.market_authority.key().ne(&market_authority_pda) {
            msg! {"market authority: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        let token_account = TokenAccount::from_account_info(ctx.reserve_vault)?;
        if token_account.mint().ne(&config.reserve_liquidity_mint) {
            msg! {"token_account_info: invalid mint"};
            return Err(ProgramError::InvalidAccountData);
        }
        if token_account.owner().ne(controller_authority) {
            msg! {"token_account_info: not owned by Controller authority PDA"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.reserve_vault.key().ne(&reserve.vault) {
            msg! {"token_account_info: mismatch with reserve"};
            return Err(ProgramError::InvalidAccountData);
        }

        if ctx.kamino_reserve_liquidity_mint.key().ne(&reserve.mint) {
            msg! {"reserve_liquidity_mint: mismatch with reserve"};
            return Err(ProgramError::InvalidAccountData);
        }

        // Verify obligation farm collateral is valid
        let obligation_farm_collateral_pda = derive_obligation_farm_address(
            ctx.reserve_farm_collateral.key(),
            ctx.obligation.key(),
            ctx.kamino_farms_program.key(),
        )?;
        if obligation_farm_collateral_pda.ne(ctx.obligation_farm_collateral.key()) {
            msg! {"Obligation farm collateral: Invalid address"}
            return Err(SvmAlmControllerErrors::InvalidPda.into());
        }

        Ok(ctx)
    }
}
