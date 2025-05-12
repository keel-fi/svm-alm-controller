use crate::{
    enums::{ControllerStatus, IntegrationStatus, PermissionStatus, ReserveStatus},
    error::SvmAlmControllerErrors,
    instructions::PullArgs,
    integrations::spl_token_swap::pull::process_pull_spl_token_swap,
    state::{Controller, Integration, Permission, Reserve},
};
use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

pub struct PullAccounts<'info> {
    pub controller: &'info AccountInfo,
    pub authority: &'info AccountInfo,
    pub permission: &'info AccountInfo,
    pub integration: &'info AccountInfo,
    pub reserve_a: &'info AccountInfo,
    pub reserve_b: &'info AccountInfo,
    pub remaining_accounts: &'info [AccountInfo],
}

impl<'info> PullAccounts<'info> {
    pub fn from_accounts(accounts: &'info [AccountInfo]) -> Result<Self, ProgramError> {
        if accounts.len() < 6 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let ctx = Self {
            controller: &accounts[0],
            authority: &accounts[1],
            permission: &accounts[2],
            integration: &accounts[3],
            reserve_a: &accounts[4],
            reserve_b: &accounts[5],
            remaining_accounts: &accounts[6..],
        };
        if !ctx.controller.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !ctx.permission.is_owned_by(&crate::ID) {
            msg! {"Permission: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration.is_owned_by(&crate::ID) {
            msg! {"Integration: wrong owner"};
            return Err(ProgramError::InvalidAccountOwner);
        }
        if !ctx.integration.is_writable() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(ctx)
    }
}

pub fn process_pull(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("pull");

    let clock = Clock::get()?;
    let ctx = PullAccounts::from_accounts(accounts)?;
    // // Deserialize the args
    let args = PullArgs::try_from_slice(instruction_data).unwrap();

    // Load in controller state
    let controller = Controller::load_and_check(ctx.controller)?;
    if controller.status != ControllerStatus::Active {
        return Err(SvmAlmControllerErrors::ControllerStatusDoesNotPermitAction.into());
    }

    // Load in the super permission account
    let permission =
        Permission::load_and_check(ctx.permission, ctx.controller.key(), ctx.authority.key())?;
    if permission.status != PermissionStatus::Active {
        return Err(SvmAlmControllerErrors::PermissionStatusDoesNotPermitAction.into());
    }

    // Load in the super permission account
    let mut integration = Integration::load_and_check_mut(ctx.integration, ctx.controller.key())?;
    if integration.status != IntegrationStatus::Active {
        return Err(SvmAlmControllerErrors::IntegrationStatusDoesNotPermitAction.into());
    }
    integration.refresh_rate_limit(clock)?;

    // Load in the reserve account for a
    let mut reserve_a = Reserve::load_and_check_mut(ctx.reserve_a, ctx.controller.key())?;
    if reserve_a.status != ReserveStatus::Active {
        return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
    }

    // Load in the reserve account for b (if applicable)
    let reserve_b = if ctx.reserve_a.key().ne(ctx.reserve_b.key()) {
        let reserve_b = Reserve::load_and_check_mut(ctx.reserve_b, ctx.controller.key())?;
        if reserve_b.status != ReserveStatus::Active {
            return Err(SvmAlmControllerErrors::ReserveStatusDoesNotPermitAction.into());
        }
        Some(reserve_b)
    } else {
        None
    };

    match args {
        PullArgs::SplTokenSwap { .. } => {
            process_pull_spl_token_swap(
                &controller,
                &permission,
                &mut integration,
                &mut reserve_a,
                &mut reserve_b.unwrap(),
                &ctx,
                &args,
            )?;
        }
        _ => return Err(ProgramError::InvalidArgument),
    }

    // Save the reserve and integration accounts
    integration.save(ctx.integration)?;
    reserve_a.save(ctx.reserve_a)?;
    if reserve_b.is_some() {
        reserve_b.unwrap().save(ctx.reserve_b)?;
    }

    Ok(())
}
