use crate::{
    define_account_struct,
    error::SvmAlmControllerErrors,
    events::{ReserveUpdateEvent, SvmAlmControllerEvent},
    instructions::InitializeReserveArgs,
    state::{nova_account::NovaAccount, Controller, Permission, Reserve},
};
use borsh::BorshDeserialize;
use pinocchio::{account_info::AccountInfo, msg, pubkey::Pubkey, ProgramResult};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::state::Mint;

define_account_struct! {
  pub struct InitializeReserveAccounts<'info> {
      payer: signer, mut;
      controller: @owner(crate::ID);
      controller_authority: empty, @owner(pinocchio_system::ID);
      authority: signer;
      permission: @owner(crate::ID);
      reserve: mut, empty, @owner(pinocchio_system::ID);
      mint;
      vault: mut;
      token_program: @pubkey(pinocchio_token::ID, pinocchio_token2022::ID);
      associated_token_program: @pubkey(pinocchio_associated_token_account::ID);
      program_id: @pubkey(crate::ID);
      system_program: @pubkey(pinocchio_system::ID);
  }
}

pub fn process_initialize_reserve(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("initialize_reserve");

    let ctx = InitializeReserveAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = InitializeReserveArgs::try_from_slice(instruction_data).unwrap();

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;

    // Validate the controller authority
    if controller.authority.ne(ctx.controller_authority.key()) {
        return Err(SvmAlmControllerErrors::InvalidControllerAuthority.into());
    }

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    // Check that super authority has permission and the permission is active
    if !permission.can_manage_integrations() {
        return Err(SvmAlmControllerErrors::UnauthorizedAction.into());
    }

    // Validate the mint
    // Load in the mint account, validating it in the process
    Mint::from_account_info(ctx.mint).unwrap();

    // Invoke the CreateIdempotent ixn for the ATA
    // Will handle both the creation or the checking, if already created
    CreateIdempotent {
        funding_account: ctx.payer,
        account: ctx.vault,
        wallet: ctx.controller_authority,
        mint: ctx.mint,
        system_program: ctx.system_program,
        token_program: ctx.token_program,
    }
    .invoke()
    .unwrap();

    // Initialize the reserve account
    let mut reserve = Reserve::init_account(
        ctx.reserve,
        ctx.payer,
        *ctx.controller.key(),
        *ctx.mint.key(),
        *ctx.vault.key(),
        args.status,
        args.rate_limit_slope,
        args.rate_limit_max_outflow,
    )?;

    // Emit the Event to record the update
    controller.emit_event(
        ctx.controller_authority,
        ctx.controller.key(),
        SvmAlmControllerEvent::ReserveUpdate(ReserveUpdateEvent {
            controller: *ctx.controller.key(),
            reserve: *ctx.reserve.key(),
            authority: *ctx.authority.key(),
            old_state: None,
            new_state: Some(reserve),
        }),
    )?;

    // Call the initial sync balance
    reserve.sync_balance(
        ctx.vault,
        ctx.controller_authority,
        ctx.controller.key(),
        &controller,
    )?;

    // Save the account state
    reserve.save(ctx.reserve)?;

    Ok(())
}
