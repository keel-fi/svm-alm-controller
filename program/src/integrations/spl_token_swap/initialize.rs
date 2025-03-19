use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, 
    msg, 
    program_error::ProgramError, 
    pubkey::Pubkey, sysvars::{clock::Clock, Sysvar}, 
};
use crate::{
    enums::{IntegrationConfig, IntegrationState}, instructions::InitializeIntegrationArgs, integrations::spl_token_swap::{config::SplTokenSwapConfig, state::SplTokenSwapState, swap_state::{SwapV1Subset, LEN_SWAP_V1_SUBSET}}, processor::InitializeIntegrationAccounts
};
use pinocchio_token::{self, state::{Mint, TokenAccount}};
use pinocchio_associated_token_account::{self, instructions::CreateIdempotent};


pub struct InitializeSplTokenSwapAccounts<'info> {
    pub swap: &'info AccountInfo,
    pub mint_a: &'info AccountInfo,
    pub mint_b: &'info AccountInfo,
    pub lp_mint: &'info AccountInfo,
    pub lp_token_account: &'info AccountInfo,
    pub mint_a_token_program: &'info AccountInfo,
    pub mint_b_token_program: &'info AccountInfo,
    pub lp_mint_token_program: &'info AccountInfo,
    pub swap_token_a: &'info AccountInfo,
    pub swap_token_b: &'info AccountInfo,
    pub swap_program: &'info AccountInfo,
    pub associated_token_program: &'info AccountInfo,
}


impl<'info> InitializeSplTokenSwapAccounts<'info> {

    pub fn from_accounts(
        account_infos: &'info [AccountInfo],
    ) -> Result<Self, ProgramError> {
        if account_infos.len() != 12 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            swap: &account_infos[0],
            mint_a: &account_infos[1],
            mint_b: &account_infos[2],
            lp_mint: &account_infos[3],
            lp_token_account: &account_infos[4],
            mint_a_token_program: &account_infos[5],
            mint_b_token_program: &account_infos[6],
            lp_mint_token_program: &account_infos[7],
            swap_token_a: &account_infos[8],
            swap_token_b: &account_infos[9],
            swap_program: &account_infos[10],
            associated_token_program: &account_infos[11],
        };
        if ctx.swap.owner().ne(ctx.swap_program.key()) {
            msg!{"pool: not owned by swap_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        // TODO: More checks on swap
        if ctx.mint_a.owner().ne(ctx.mint_a_token_program.key()) {
            msg!{"mint_a: not owned by mint_a_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.mint_b.owner().ne(ctx.mint_b_token_program.key()) {
            msg!{"mint_b: not owned by mint_b_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.lp_mint.owner().ne(ctx.lp_mint_token_program.key()) {
            msg!{"lp_mint: not owned by lp_mint_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.lp_mint.is_writable() {
            msg!{"lp_mint: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if !ctx.swap.is_writable() {
            msg!{"pool: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.mint_a_token_program.key().ne(&pinocchio_token::ID){ // TODO: Allow token 2022
            msg!{"mint_a_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.mint_b_token_program.key().ne(&pinocchio_token::ID){ // TODO: Allow token 2022
            msg!{"mint_b_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.lp_mint_token_program.key().ne(&pinocchio_token::ID){ // TODO: Allow token 2022
            msg!{"lp_mint_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if ctx.associated_token_program.key().ne(&pinocchio_associated_token_account::ID) { 
            msg!{"associated_token_program: invalid address"};
            return Err(ProgramError::IncorrectProgramId);
        }
        if !ctx.lp_token_account.is_writable() {
            msg!{"lp_token_account: not mutable"};
            return Err(ProgramError::InvalidAccountData);
        }
        if ctx.lp_token_account.owner().ne(ctx.lp_mint_token_program.key()) && ctx.lp_token_account.owner().ne(&pinocchio_system::ID) {
            msg!{"lp_token_account: not owned by token_program or system_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.swap_token_a.owner().ne(ctx.mint_a_token_program.key()) {
            msg!{"swap_token_a: not owned by mint_a_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if ctx.swap_token_b.owner().ne(ctx.mint_b_token_program.key()) {
            msg!{"swap_token_b: not owned by mint_b_token_program"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(ctx)
    }
 

}




pub fn process_initialize_spl_token_swap(
    outer_ctx: &InitializeIntegrationAccounts,
    _outer_args: &InitializeIntegrationArgs
) -> Result<(IntegrationConfig, IntegrationState), ProgramError> {
    msg!("process_initialize_spl_token_swap");

    let inner_ctx = InitializeSplTokenSwapAccounts::from_accounts(outer_ctx.remaining_accounts)?;

    // Load in the mint accounts, validating it in the process
    Mint::from_account_info(inner_ctx.mint_a).unwrap();
    Mint::from_account_info(inner_ctx.mint_b).unwrap();
    let lp_mint = Mint::from_account_info(inner_ctx.lp_mint).unwrap();

    // Load in the Pool state and verify the accounts 
    //  w.r.t it's stored state
    let swap_data = inner_ctx.swap.try_borrow_data()?;
    let swap_state = SwapV1Subset::try_from_slice(&swap_data[1..LEN_SWAP_V1_SUBSET+1]).unwrap();

    if swap_state.token_a_mint.ne(inner_ctx.mint_a.key()) {
        msg!{"mint_a: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }
    if swap_state.token_b_mint.ne(inner_ctx.mint_b.key()) {
        msg!{"mint_b: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }
    if swap_state.pool_mint.ne(inner_ctx.lp_mint.key()) {
        msg!{"lp_mint: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }
    if swap_state.token_a.ne(inner_ctx.swap_token_a.key()) {
        msg!{"swap_token_a: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }
    if swap_state.token_b.ne(inner_ctx.swap_token_b.key()) {
        msg!{"swap_token_b: does not match swap state"};
        return Err(ProgramError::InvalidAccountData);
    }

    // Invoke the CreateIdempotent ixn for the lp_token_account (ATA)
    // Will handle both the creation or the checking, if already created
    CreateIdempotent{
        funding_account: outer_ctx.payer_info,
        account: inner_ctx.lp_token_account,
        wallet: outer_ctx.controller_info,
        mint: inner_ctx.lp_mint,
        system_program: outer_ctx.system_program,
        token_program: inner_ctx.lp_mint_token_program,
    }.invoke().unwrap();
    
    // Create the Config
    let config = IntegrationConfig::SplTokenSwap(
        SplTokenSwapConfig {
            program: Pubkey::from(*inner_ctx.swap_program.key()),
            swap: Pubkey::from(*inner_ctx.swap.key()),
            mint_a: Pubkey::from(*inner_ctx.mint_a.key()),
            mint_b: Pubkey::from(*inner_ctx.mint_b.key()),
            lp_mint: Pubkey::from(*inner_ctx.lp_mint.key()),
            lp_token_account: Pubkey::from(*inner_ctx.lp_token_account.key()),
        }
    );

    // Get the current slot and time
    let clock = Clock::get()?;
    
    // Load in the vault, since it could have an opening balance
    let lp_token_account = TokenAccount::from_account_info(inner_ctx.lp_token_account)?;
    let last_balance_lp = lp_token_account.amount() as u128;

    // If it has an opening balance, then calculate the proportional ownership in the swap vaults
    let mut last_balance_a = 0u64;
    let mut last_balance_b = 0u64;
    if last_balance_lp > 0 {
        let swap_token_a = TokenAccount::from_account_info(inner_ctx.swap_token_a)?;
        let swap_token_b = TokenAccount::from_account_info(inner_ctx.swap_token_b)?;
        let lp_mint_supply = lp_mint.supply() as u128; 
        last_balance_a = (swap_token_a.amount() as u128 * last_balance_lp / lp_mint_supply) as u64;
        last_balance_b = (swap_token_b.amount() as u128 * last_balance_lp / lp_mint_supply) as u64;
    }

    // Create the initial integration state
    let state = IntegrationState::SplTokenSwap(
        SplTokenSwapState {
            last_refresh_timestamp: clock.unix_timestamp,
            last_refresh_slot: clock.slot,
            last_balance_a: last_balance_a,
            last_balance_b: last_balance_b,
            last_balance_lp: last_balance_lp as u64,
            _padding: [0u8;8]
        }
    );

    Ok((config, state))

}

