use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    define_account_struct,
    enums::IntegrationConfig,
    integrations::psm_swap::{
        constants::PSM_SWAP_PROGRAM_ID,
        psm_swap_state::{PsmPool, Token},
    },
    state::Reserve,
};

define_account_struct! {
    pub struct PushPullPsmSwapAccounts<'info> {
        psm_pool: @owner(PSM_SWAP_PROGRAM_ID);
        psm_token: @owner(PSM_SWAP_PROGRAM_ID);
        psm_token_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        reserve_vault: mut @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
        psm_swap_program: @pubkey(PSM_SWAP_PROGRAM_ID);
    }
}

impl<'info> PushPullPsmSwapAccounts<'info> {
    pub fn checked_from_accounts(
        controller_authority: &Pubkey,
        config: &IntegrationConfig,
        account_infos: &'info [AccountInfo],
        reserve: &Reserve,
    ) -> Result<Self, ProgramError> {
        let ctx = PushPullPsmSwapAccounts::from_accounts(account_infos)?;
        let config = match config {
            IntegrationConfig::PsmSwap(psm_config) => psm_config,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        config.check_accounts(ctx.psm_token, ctx.psm_pool, ctx.mint)?;

        // validate reserve matches mint
        if reserve.mint.ne(ctx.mint.key()) {
            msg!("mint: does not match reserve");
            return Err(ProgramError::InvalidAccountData);
        }

        // validate token_program is correct
        if ctx.token_program.key().ne(ctx.mint.owner()) {
            msg! {"token_program: mismatch with mint"};
            return Err(ProgramError::InvalidAccountData);
        }

        // validate psm_pool.liquidity_owner is controller authority
        let psm_pool_data = ctx.psm_pool.try_borrow_data()?;
        let psm_pool = PsmPool::try_from_slice(&psm_pool_data)?;

        if psm_pool.liquidity_owner.ne(controller_authority) {
            msg! {"psm_pool: mismatch with controller_authority"};
            return Err(ProgramError::InvalidAccountData);
        }

        // validate psm_token_vault matches the psm_token
        let psm_token_data = ctx.psm_token.try_borrow_data()?;
        let psm_token = Token::try_from_slice(&psm_token_data)?;

        if psm_token.vault.ne(ctx.psm_token_vault.key()) {
            msg! {"psm_token_vault: mismatch with psm_token"};
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(ctx)
    }
}
