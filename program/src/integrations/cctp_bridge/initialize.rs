use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::cctp_bridge::{
        cctp_state::{LocalToken, RemoteTokenMessenger},
        config::CctpBridgeConfig,
        constants::{CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID, CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID},
        state::CctpBridgeState,
    },
    processor::{shared::validate_mint_extensions, InitializeIntegrationAccounts},
};
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};

define_account_struct! {
  pub struct InitializeCctpBridgeAccounts<'info> {
      mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
      local_token;
      remote_token_messenger;
      cctp_message_transmitter @pubkey(CCTP_MESSAGE_TRANSMITTER_PROGRAM_ID);
      cctp_token_messenger_minter @pubkey(CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID);
  }
}

impl<'info> InitializeCctpBridgeAccounts<'info> {
    pub fn checked_from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = InitializeCctpBridgeAccounts::from_accounts(account_infos)?;

        // Ensure the mint has valid T22 extensions.
        validate_mint_extensions(ctx.mint, &[])?;

        if !ctx
            .local_token
            .is_owned_by(ctx.cctp_token_messenger_minter.key())
        {
            msg! {"local_mint: not owned by cctp_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx
            .remote_token_messenger
            .is_owned_by(ctx.cctp_token_messenger_minter.key())
        {
            msg! {"remote_token_messenger: not owned by cctp_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(ctx)
    }
}

pub fn process_initialize_cctp_bridge(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_cctp_bridge");

    let inner_ctx =
        InitializeCctpBridgeAccounts::checked_from_accounts(outer_ctx.remaining_accounts)?;

    let (destination_address, destination_domain) = match outer_args.inner_args {
        InitializeArgs::CctpBridge {
            destination_address,
            destination_domain,
        } => (destination_address, destination_domain),
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Load in the CCTP Local Token Account and verify the mint matches
    let local_token =
        LocalToken::deserialize(&mut &*inner_ctx.local_token.try_borrow_data()?).map_err(|e| e)?;
    if local_token.mint.ne(inner_ctx.mint.key()) {
        msg! {"mint: does not match local_token state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Load in the CCTP RemoteTokenMessenger account
    let remote_token_messenger = RemoteTokenMessenger::deserialize(
        &mut &*inner_ctx.remote_token_messenger.try_borrow_data()?,
    )
    .map_err(|e| e)?;
    if remote_token_messenger.domain.ne(&destination_domain) {
        msg! {"destination_domain: does not match remote_token_messenger state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Create the Config
    let config = IntegrationConfig::CctpBridge(CctpBridgeConfig {
        cctp_token_messenger_minter: Pubkey::from(*inner_ctx.cctp_token_messenger_minter.key()),
        cctp_message_transmitter: Pubkey::from(*inner_ctx.cctp_message_transmitter.key()),
        mint: Pubkey::from(*inner_ctx.mint.key()),
        destination_address: Pubkey::from(destination_address),
        destination_domain,
        _padding: [0u8; 92],
    });

    // Create the initial integration state
    let state = IntegrationState::CctpBridge(CctpBridgeState {
        _padding: [0u8; 48],
    });

    Ok((config, state))
}
