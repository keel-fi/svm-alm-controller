use crate::{
    define_account_struct,
    enums::{IntegrationConfig, IntegrationState},
    error::SvmAlmControllerErrors,
    instructions::{InitializeArgs, InitializeIntegrationArgs},
    integrations::lz_bridge::{
        config::LzBridgeConfig,
        lz_state::{OFTStore, PeerConfig, OFT_PEER_CONFIG_SEED},
        state::LzBridgeState,
    },
    processor::{shared::validate_mint_extensions, InitializeIntegrationAccounts},
};
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
};
use pinocchio_token2022::extensions::ExtensionType;

define_account_struct! {
    pub struct InitializeLzBridgeAccounts<'info> {
        mint: @owner(pinocchio_token::ID, pinocchio_token2022::ID);
        // No addresses enforced for the following accounts in order to support
        // multiple OFTs. This is safe since only a Permissioned address may
        // create this Integration.
        oft_store;
        peer_config;
        oft_program;
        token_escrow;
    }
}

impl<'info> InitializeLzBridgeAccounts<'info> {
    pub fn checked_from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        let ctx = Self::from_accounts(account_infos)?;

        // Ensure the mint has valid T22 extensions.
        // Block the usage of TransferFees as it's not needed. If it's deemed
        // valuable in the future for LZ OFT integrations, then we can refactor
        // and hopefully the CPI limits have increased to enable direct OFT Send CPIs.
        validate_mint_extensions(ctx.mint, &[ExtensionType::TransferFeeConfig])?;

        if !ctx.oft_store.is_owned_by(ctx.oft_program.key()) {
            msg! {"oft_store: not owned by oft_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.peer_config.is_owned_by(ctx.oft_program.key()) {
            msg! {"peer_config: not owned by oft_program"};
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

    let inner_ctx =
        InitializeLzBridgeAccounts::checked_from_accounts(outer_ctx.remaining_accounts)?;

    let (desination_address, destination_eid) = match outer_args.inner_args {
        InitializeArgs::LzBridge {
            destination_address,
            destination_eid,
        } => (destination_address, destination_eid),
        _ => return Err(ProgramError::InvalidArgument),
    };

    // Load in the LZ OFT Store Account and verify the mint matches
    let oft_store =
        OFTStore::deserialize(&mut &*inner_ctx.oft_store.try_borrow_data()?).map_err(|e| e)?;
    if oft_store.token_mint.ne(inner_ctx.mint.key()) {
        msg! {"mint: does not match oft_store state"};
        return Err(ProgramError::InvalidAccountData);
    }
    if oft_store.token_escrow.ne(inner_ctx.token_escrow.key()) {
        msg! {"token_escrow: does not match oft_store state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Check the PDA of the peer_config exists for this desination_eid
    let (expected_peer_config_pda, _bump) = try_find_program_address(
        &[
            OFT_PEER_CONFIG_SEED,
            inner_ctx.oft_store.key().as_ref(),
            destination_eid.to_be_bytes().as_ref(),
        ],
        inner_ctx.oft_program.key(),
    )
    .ok_or(ProgramError::InvalidSeeds)?;
    if inner_ctx.peer_config.key().ne(&expected_peer_config_pda) {
        msg! {"peer_config: expected PDA for destination_eid and oft store do not match"};
        return Err(SvmAlmControllerErrors::InvalidPda.into());
    }
    // Load in the LZ Peer Config Account (if it doesn't load it's not configured)
    PeerConfig::deserialize(&mut &*inner_ctx.peer_config.try_borrow_data()?).map_err(|e| e)?;

    // Create the Config
    let config = IntegrationConfig::LzBridge(LzBridgeConfig {
        program: Pubkey::from(*inner_ctx.oft_program.key()),
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
        push_in_flight: false,
        _padding: [0u8; 47],
    });

    Ok((config, state))
}
