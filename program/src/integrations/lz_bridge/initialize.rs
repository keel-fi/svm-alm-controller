use crate::{
    enums::{IntegrationConfig, IntegrationState},
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::lz_bridge::{
        config::LzBridgeConfig,
        lz_state::{OFTStore, PeerConfig, OFT_PEER_CONFIG_SEED},
        state::LzBridgeState,
    },
    processor::InitializeIntegrationAccounts,
};
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use solana_program::pubkey::Pubkey as SolanaPubkey;

pub struct InitializeLzBridgeAccounts<'info> {
    pub mint: &'info AccountInfo,
    pub oft_store: &'info AccountInfo,
    pub peer_config: &'info AccountInfo,
    pub lz_program: &'info AccountInfo,
    pub token_escrow: &'info AccountInfo,
}

impl<'info> InitializeLzBridgeAccounts<'info> {
    pub fn from_accounts(account_infos: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if account_infos.len() != 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            mint: &account_infos[0],
            oft_store: &account_infos[1],
            peer_config: &account_infos[2],
            lz_program: &account_infos[3],
            token_escrow: &account_infos[4],
        };
        // TODO: Do we need to check LZ program against a const?
        if !ctx.oft_store.is_owned_by(ctx.lz_program.key()) {
            msg! {"oft_store: not owned by cctp_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.peer_config.is_owned_by(ctx.lz_program.key()) {
            msg! {"peer_config: not owned by cctp_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.mint.is_owned_by(&pinocchio_token::ID) {
            // TODO: Allow token 2022
            msg! {"mint: not owned by token program"};
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(ctx)
    }
}

pub fn process_initialize_lz_bridge(
    outer_ctx: &InitializeIntegrationAccounts,
    outer_args: &InitializeIntegrationArgs,
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_lz_bridge");

    let inner_ctx = InitializeLzBridgeAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    let (desination_address, destination_eid) = match outer_args.inner_args {
        InitializeArgs::LzBridge {
            desination_address,
            destination_eid,
        } => (desination_address, destination_eid),
        _ => return Err(ProgramError::InvalidArgument),
    };
    msg!("a");

    // Load in the LZ OFT Store Account and verify the mint matches
    let oft_store =
        OFTStore::deserialize(&mut &*inner_ctx.oft_store.try_borrow_data()?).map_err(|e| e)?;
    if oft_store.token_mint.ne(inner_ctx.mint.key()) {
        msg! {"mint: does not match oft_store state"};
        return Err(ProgramError::InvalidAccountData);
    }
    msg!("b");

    // Check the PDA of the peer_config exists for this desination_eid
    let (expected_peer_config_pda, _bump) = SolanaPubkey::find_program_address(
        &[
            OFT_PEER_CONFIG_SEED,
            inner_ctx.oft_store.key().as_ref(),
            destination_eid.to_be_bytes().as_ref(),
        ],
        &SolanaPubkey::from(*inner_ctx.lz_program.key()),
    );
    if inner_ctx
        .peer_config
        .key()
        .ne(&expected_peer_config_pda.to_bytes())
    {
        msg! {"peer_config: expected PDA for destination_eid and oft store do not match"};
        return Err(ProgramError::InvalidSeeds);
    }
    // Load in the LZ Peer Config Account (if it doesn't load it's not configured)
    PeerConfig::deserialize(&mut &*inner_ctx.peer_config.try_borrow_data()?).map_err(|e| e)?;

    // Create the Config
    let config = IntegrationConfig::LzBridge(LzBridgeConfig {
        program: Pubkey::from(*inner_ctx.lz_program.key()),
        mint: Pubkey::from(*inner_ctx.mint.key()),
        oft_store: Pubkey::from(*inner_ctx.oft_store.key()),
        peer_config: Pubkey::from(*inner_ctx.peer_config.key()),
        token_escrow: Pubkey::from(*inner_ctx.token_escrow.key()),
        destination_address: Pubkey::from(desination_address),
        destination_eid,
        _padding: [0u8; 28],
    });

    // Create the initial integration state
    let state = IntegrationState::LzBridge(LzBridgeState {
        _padding: [0u8; 48],
    });

    Ok((config, state))
}
