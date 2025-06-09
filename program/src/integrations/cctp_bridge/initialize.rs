use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::cctp_bridge::{
        cctp_state::{LocalToken, RemoteTokenMessenger},
        config::CctpBridgeConfig,
        state::CctpBridgeState,
    },
    processor::InitializeIntegrationAccounts,
};
use pinocchio::{msg, program_error::ProgramError, pubkey::Pubkey};

define_account_struct! {
  pub struct InitializeCctpBridgeAccounts<'info> {
      mint: @owner(pinocchio_token::ID);
      local_token;
      remote_token_messenger;
      cctp_message_transmitter;
      cctp_token_messenger_minter;
  }
}

pub fn process_initialize_cctp_bridge(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_cctp_bridge");

    let inner_ctx = InitializeCctpBridgeAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    // Additional checks
    if !inner_ctx
        .local_token
        .is_owned_by(inner_ctx.cctp_token_messenger_minter.key())
    {
        msg! {"local_mint: not owned by cctp_program"};
        return Err(ProgramError::InvalidAccountOwner);
    }
    if !inner_ctx
        .remote_token_messenger
        .is_owned_by(inner_ctx.cctp_token_messenger_minter.key())
    {
        msg! {"remote_token_messenger: not owned by cctp_program"};
        return Err(ProgramError::InvalidAccountOwner);
    }

    let (desination_address, desination_domain) = match outer_args.inner_args {
        InitializeArgs::CctpBridge {
            desination_address,
            desination_domain,
        } => (desination_address, desination_domain),
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Load in the CCTP Local Token Account and verify the mint matches
    let local_token =
        LocalToken::deserialize(&mut &*inner_ctx.local_token.try_borrow_data()?).map_err(|e| e)?;
    if local_token.mint.ne(inner_ctx.mint.key()) {
        msg! {"mint: does not match local_token state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Load in the CCTP RemoteTokenMessenger account and verify the mint matches
    let remote_token_messenger = RemoteTokenMessenger::deserialize(
        &mut &*inner_ctx.remote_token_messenger.try_borrow_data()?,
    )
    .map_err(|e| e)?;
    if remote_token_messenger.domain.ne(&desination_domain) {
        msg! {"desination_domain: does not match remote_token_messenger state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Create the Config
    let config = IntegrationConfig::CctpBridge(CctpBridgeConfig {
        cctp_token_messenger_minter: Pubkey::from(*inner_ctx.cctp_token_messenger_minter.key()),
        cctp_message_transmitter: Pubkey::from(*inner_ctx.cctp_message_transmitter.key()),
        mint: Pubkey::from(*inner_ctx.mint.key()),
        destination_address: Pubkey::from(desination_address),
        destination_domain: desination_domain,
        _padding: [0u8; 60],
    });

    // Create the initial integration state
    let state = IntegrationState::CctpBridge(CctpBridgeState {
        _padding: [0u8; 48],
    });

    Ok((config, state))
}
