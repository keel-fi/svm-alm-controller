use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::InitializeIntegrationArgs,
    integrations::spl_token_external::{
        config::SplTokenExternalConfig, state::SplTokenExternalState,
    },
    processor::InitializeIntegrationAccounts,
};
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_associated_token_account::{self, instructions::CreateIdempotent};
use pinocchio_token::{self, state::Mint};

define_account_struct! {
    pub struct InitializeSplTokenExternalAccounts<'info> {
        mint: @owner(pinocchio_token::ID);
        recipient;
        token_account: mut;
        token_program: @pubkey(pinocchio_token::ID);
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
    }
}

impl<'info> InitializeSplTokenExternalAccounts<'info> {
    pub fn checked_from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;

        if !ctx.token_account.is_owned_by(ctx.token_program.key())
            && !ctx.token_account.is_owned_by(&pinocchio_system::ID)
        {
            msg! {"token_account: not owned by token_program or system_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(ctx)
    }
}

pub fn process_initialize_spl_token_external(
    outer_ctx: &InitializeIntegrationAccounts,
    _outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_spl_token_external");

    let inner_ctx =
        InitializeSplTokenExternalAccounts::checked_from_accounts(outer_ctx.remaining_accounts)?;

    // Load in the mint account, validating it in the process
    Mint::from_account_info(inner_ctx.mint).unwrap();

    // Invoke the CreateIdempotent ixn for the token_accout (ATA)
    // Will handle both the creation or the checking, if already created
    CreateIdempotent {
        funding_account: outer_ctx.payer,
        account: inner_ctx.token_account,
        wallet: inner_ctx.recipient,
        mint: inner_ctx.mint,
        system_program: outer_ctx.system_program,
        token_program: inner_ctx.token_program,
    }
    .invoke()
    .unwrap();

    // Create the Config
    let config = IntegrationConfig::SplTokenExternal(SplTokenExternalConfig {
        program: Pubkey::from(*inner_ctx.token_program.key()),
        mint: Pubkey::from(*inner_ctx.mint.key()),
        recipient: Pubkey::from(*inner_ctx.recipient.key()),
        token_account: Pubkey::from(*inner_ctx.token_account.key()),
        _padding: [0u8; 64],
    });

    // Create the initial integration state
    let state = IntegrationState::SplTokenExternal(SplTokenExternalState {
        _padding: [0u8; 48],
    });

    Ok((config, state))
}
