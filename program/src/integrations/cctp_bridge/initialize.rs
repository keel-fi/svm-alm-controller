use pinocchio::{
    account_info::AccountInfo, 
    msg, 
    program_error::ProgramError, 
    pubkey::Pubkey, 
};
use crate::{
    enums::{IntegrationConfig, IntegrationState}, instructions::{InitializeArgs, InitializeIntegrationArgs}, 
    integrations::cctp_bridge::{cctp_state::{LocalToken, RemoteTokenMessenger}, config::CctpBridgeConfig, state::CctpBridgeState}, 
    processor::InitializeIntegrationAccounts
};


pub struct InitializeCctpBridgeAccounts<'info> {
    pub mint: &'info AccountInfo,
    pub local_mint: &'info AccountInfo,
    pub remote_token_messenger: &'info AccountInfo,
    pub cctp_program: &'info AccountInfo,
}


impl<'info> InitializeCctpBridgeAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            mint: &account_infos[0],
            local_mint: &account_infos[1],
            remote_token_messenger: &account_infos[2],
            cctp_program: &account_infos[3],
        };
        if ctx.local_mint.owner().ne(ctx.cctp_program.key()) {
            msg!{"local_mint: not owned by cctp_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.remote_token_messenger.owner().ne(ctx.cctp_program.key()) {
            msg!{"remote_token_messenger: not owned by cctp_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.mint.owner().ne(&pinocchio_token::ID){ // TODO: Allow token 2022
            msg!{"mint: not owned by token program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        
        Ok(ctx)
    }
 

}


pub fn process_initialize_cctp_bridge(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_cctp_bridge");

    let inner_ctx = InitializeCctpBridgeAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    let (desination_address, desination_domain) = match outer_args.inner_args {
        InitializeArgs::CctpBridge { desination_address, desination_domain } => (desination_address, desination_domain),
        _ => return Err(ProgramError::InvalidArgument)
    };
    
    // Load in the CCTP Local Token Account and verify the mint matches
    let local_mint= LocalToken::deserialize(&mut &*inner_ctx.local_mint.try_borrow_data()?).unwrap();
    if local_mint.mint.ne(inner_ctx.mint.key()) {
        msg!{"mint: does not match local_mint state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Load in the CCTP RemoteTokenMessenger account and verify the mint matches
    let remote_token_messenger= RemoteTokenMessenger::deserialize(&mut &*inner_ctx.remote_token_messenger.try_borrow_data()?).unwrap();
    if remote_token_messenger.domain.ne(&desination_domain) {
        msg!{"desination_domain: does not match remote_token_messenger state"};
        return Err(ProgramError::InvalidAccountData);
    }
  
    // Create the Config
    let config = IntegrationConfig::CctpBridge(
        CctpBridgeConfig {
            program: Pubkey::from(*inner_ctx.cctp_program.key()),
            mint: Pubkey::from(*inner_ctx.mint.key()),
            destination_address: Pubkey::from(desination_address),
            destination_domain: desination_domain,
            _padding: [0u8; 92]
        }
    );

    // Create the initial integration state
    let state = IntegrationState::CctpBridge(
        CctpBridgeState {
            _padding: [0u8;48]
        }
    );

    Ok((config, state))

}

