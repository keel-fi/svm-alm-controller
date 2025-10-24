/* Kamino LendingProcessor implementation */

use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
};
use pinocchio_token_interface::TokenAccount;

use crate::{
    constants::CONTROLLER_AUTHORITY_SEED,
    enums::IntegrationState,
    integrations::{
        kamino::{
            balance::get_kamino_lending_balance,
            cpi::{DepositReserveLiquidityAndObligationCollateralV2, WithdrawObligationCollateralAndRedeemReserveCollateralV2},
            klend_protocol_state::KaminoReserve,
            shared_sync::sync_kamino_liquidity_value,
            validations::PushPullKaminoAccounts,
        },
        shared::lending_processor::{LendingContext, LendingOperationResult, LendingProcessor},
    },
    state::Integration,
};

pub struct KaminoLendingProcessor;

impl LendingProcessor for KaminoLendingProcessor {
    fn get_current_balance(&self, ctx: &LendingContext) -> Result<u64, ProgramError> {
        match &ctx.integration.state {
            IntegrationState::Kamino(state) => Ok(state.balance),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    fn update_integration_state(&self, integration: &mut Integration, new_balance: u64) -> Result<(), ProgramError> {
        match &mut integration.state {
            IntegrationState::Kamino(state) => {
                state.balance = new_balance;
                Ok(())
            }
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    fn execute_deposit(&self, ctx: &mut LendingContext, amount: u64) -> Result<LendingOperationResult, ProgramError> {
        use pinocchio::instruction::{Seed, Signer};

        // Parse accounts
        let accounts = PushPullKaminoAccounts::checked_from_accounts(
            ctx.controller_authority.key(),
            &ctx.integration.config,
            &[], // This would need to be passed from the caller
            ctx.reserve,
        )?;

        // Track balances before operation
        let liquidity_amount_before = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };

        let liquidity_value_before = get_kamino_lending_balance(accounts.kamino_reserve, accounts.obligation)?;

        // Execute deposit
        DepositReserveLiquidityAndObligationCollateralV2 {
            owner: ctx.controller_authority,
            obligation: accounts.obligation,
            lending_market: accounts.market,
            market_authority: accounts.market_authority,
            kamino_reserve: accounts.kamino_reserve,
            reserve_liquidity_mint: accounts.kamino_reserve_liquidity_mint,
            reserve_liquidity_supply: accounts.kamino_reserve_liquidity_supply,
            reserve_collateral_mint: accounts.kamino_reserve_collateral_mint,
            reserve_collateral_supply: accounts.kamino_reserve_collateral_supply,
            user_source_liquidity: accounts.reserve_vault,
            placeholder_user_destination_collateral: accounts.kamino_program,
            collateral_token_program: accounts.collateral_token_program,
            liquidity_token_program: accounts.liquidity_token_program,
            instruction_sysvar: accounts.instruction_sysvar_account,
            obligation_farm_user_state: accounts.obligation_farm_collateral,
            reserve_farm_state: accounts.reserve_farm_collateral,
            farms_program: accounts.kamino_farms_program,
            liquidity_amount: amount,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(ctx.controller_pubkey),
            Seed::from(&[ctx.controller.authority_bump]),
        ])])?;

        // Calculate deltas
        let liquidity_amount_after = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };
        let reserve_delta = liquidity_amount_before.saturating_sub(liquidity_amount_after);

        let liquidity_value_after = get_kamino_lending_balance(accounts.kamino_reserve, accounts.obligation)?;
        let integration_delta = liquidity_value_after.saturating_sub(liquidity_value_before);

        Ok(LendingOperationResult {
            integration_delta,
            reserve_delta,
            new_balance: liquidity_value_after,
        })
    }

    fn execute_withdrawal(&self, ctx: &mut LendingContext, amount: u64) -> Result<LendingOperationResult, ProgramError> {
        use pinocchio::instruction::{Seed, Signer};

        // Parse accounts
        let accounts = PushPullKaminoAccounts::checked_from_accounts(
            ctx.controller_authority.key(),
            &ctx.integration.config,
            &[], // This would need to be passed from the caller
            ctx.reserve,
        )?;

        // Track balances before operation
        let liquidity_amount_before = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };

        let liquidity_value_before = get_kamino_lending_balance(accounts.kamino_reserve, accounts.obligation)?;

        // Convert liquidity amount to collateral amount
        let kamino_reserve_data = accounts.kamino_reserve.try_borrow_data()?;
        let kamino_reserve_state = KaminoReserve::try_from_slice(&kamino_reserve_data)?;
        let collateral_amount = kamino_reserve_state.liquidity_to_collateral(amount);
        drop(kamino_reserve_data);

        // Execute withdrawal
        WithdrawObligationCollateralAndRedeemReserveCollateralV2 {
            owner: ctx.controller_authority,
            obligation: accounts.obligation,
            lending_market: accounts.market,
            market_authority: accounts.market_authority,
            kamino_reserve: accounts.kamino_reserve,
            reserve_liquidity_mint: accounts.kamino_reserve_liquidity_mint,
            reserve_collateral_supply: accounts.kamino_reserve_collateral_supply,
            reserve_collateral_mint: accounts.kamino_reserve_collateral_mint,
            reserve_liquidity_supply: accounts.kamino_reserve_liquidity_supply,
            user_liquidity_destination: accounts.reserve_vault,
            placeholder_user_destination_collateral: accounts.kamino_program,
            collateral_token_program: accounts.collateral_token_program,
            liquidity_token_program: accounts.liquidity_token_program,
            instruction_sysvar: accounts.instruction_sysvar_account,
            obligation_farm_user_state: accounts.obligation_farm_collateral,
            reserve_farm_state: accounts.reserve_farm_collateral,
            farms_program: accounts.kamino_farms_program,
            collateral_amount,
        }
        .invoke_signed(&[Signer::from(&[
            Seed::from(CONTROLLER_AUTHORITY_SEED),
            Seed::from(ctx.controller_pubkey),
            Seed::from(&[ctx.controller.authority_bump]),
        ])])?;

        // Calculate deltas
        let liquidity_amount_after = {
            let vault = TokenAccount::from_account_info(accounts.reserve_vault)?;
            vault.amount()
        };
        let reserve_delta = liquidity_amount_after.saturating_sub(liquidity_amount_before);

        let liquidity_value_after = get_kamino_lending_balance(accounts.kamino_reserve, accounts.obligation)?;
        let integration_delta = liquidity_value_before.saturating_sub(liquidity_value_after);

        Ok(LendingOperationResult {
            integration_delta,
            reserve_delta,
            new_balance: liquidity_value_after,
        })
    }

    fn sync_balance(&self, ctx: &mut LendingContext) -> Result<u64, ProgramError> {
        // Parse accounts
        let accounts = PushPullKaminoAccounts::checked_from_accounts(
            ctx.controller_authority.key(),
            &ctx.integration.config,
            &[], // This would need to be passed from the caller
            ctx.reserve,
        )?;

        sync_kamino_liquidity_value(
            ctx.controller,
            ctx.integration,
            ctx.integration_pubkey,
            ctx.controller_pubkey,
            ctx.controller_authority,
            accounts.kamino_reserve_liquidity_mint.key(),
            accounts.kamino_reserve,
            accounts.obligation,
        )
    }

    fn get_reserve_vault(&self, _ctx: &LendingContext) -> Result<&AccountInfo, ProgramError> {
        // This is a placeholder - in practice, the reserve vault would be passed
        // from the caller or stored in the context
        Err(ProgramError::InvalidAccountData)
    }
}
