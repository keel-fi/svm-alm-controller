use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::InitializeIntegrationArgs,
    integrations::spl_token_external::{
        config::SplTokenExternalConfig, state::SplTokenExternalState,
    },
    processor::{shared::validate_mint_extensions, InitializeIntegrationAccounts},
};
use pinocchio::{msg, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_associated_token_account::{self, instructions::CreateIdempotent};
use pinocchio_token_interface::Mint;

define_account_struct! {
    pub struct InitializeSplTokenExternalAccounts<'info> {
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        recipient;
        token_account: mut, @owner(pinocchio_token::ID, pinocchio_token2022::ID, pinocchio_system::ID);
        token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
        associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
    }
}

pub fn process_initialize_spl_token_external(
    outer_ctx: &InitializeIntegrationAccounts,
    _outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_spl_token_external");

    let inner_ctx =
        InitializeSplTokenExternalAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    // Load in the mint account, validating it in the process
    Mint::from_account_info(inner_ctx.mint)?;
    validate_mint_extensions(inner_ctx.mint)?;

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
    .invoke()?;

    // Create the Config
    let config = IntegrationConfig::SplTokenExternal(SplTokenExternalConfig {
        program: Pubkey::from(*inner_ctx.token_program.key()),
        mint: Pubkey::from(*inner_ctx.mint.key()),
        recipient: Pubkey::from(*inner_ctx.recipient.key()),
        token_account: Pubkey::from(*inner_ctx.token_account.key()),
        _padding: [0u8; 96],
    });

    // Create the initial integration state
    let state = IntegrationState::SplTokenExternal(SplTokenExternalState {
        _padding: [0u8; 48],
    });

    Ok((config, state))
}
