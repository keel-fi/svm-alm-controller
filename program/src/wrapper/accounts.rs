use pinocchio::{
    account_info::{AccountInfo, Ref},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use pinocchio_token::state::Mint;

use crate::{
    state::{Controller, Oracle, Permission, Reserve},
    wrapper::WrappedAccount,
};

pub struct MintAccount<'info> {
    info: &'info AccountInfo,
    inner: Ref<'info, Min
}

impl<'info> WrappedAccount<'info> for MintAccount<'info> {
    type Target = Mint;
    type Args = ();

    fn new(info: &'info AccountInfo) -> Result<Self, ProgramError> {
        // owner already checked inside `from_account_info`
        Ok(Self {
            info,
            inner: Mint::from_account_info(info)?,
        })
    }

    fn new_with_args(info: &'info AccountInfo, _args: Self::Args) -> Result<Self, ProgramError> {
        Self::new(info)
    }

    fn info(&self) -> &'info AccountInfo {
        self.info
    }

    fn inner(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct OracleAccount<'info> {
    info: &'info AccountInfo,
    inner: Oracle,
}

impl<'info> WrappedAccount<'info> for OracleAccount<'info> {
    type Target = Oracle;
    type Args = ();

    fn new(info: &'info AccountInfo) -> Result<Self, ProgramError> {
        Ok(Self {
            info,
            inner: Oracle::load_and_check(info)?,
        })
    }

    fn new_with_args(info: &'info AccountInfo, _args: Self::Args) -> Result<Self, ProgramError> {
        Self::new(info)
    }

    fn info(&self) -> &'info AccountInfo {
        self.info
    }

    fn inner(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ControllerAccount<'info> {
    info: &'info AccountInfo,
    inner: Controller,
}

impl<'info> WrappedAccount<'info> for ControllerAccount<'info> {
    type Target = Controller;
    type Args = ();

    fn new(info: &'info AccountInfo) -> Result<Self, ProgramError> {
        Ok(Self {
            info,
            inner: Controller::load_and_check(info)?,
        })
    }

    fn new_with_args(info: &'info AccountInfo, _args: Self::Args) -> Result<Self, ProgramError> {
        Self::new(info)
    }

    fn info(&self) -> &'info AccountInfo {
        self.info
    }

    fn inner(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct PermissionAccount<'info> {
    info: &'info AccountInfo,
    inner: Permission,
}
pub struct PermissionArgs<'info> {
    pub controller: &'info Pubkey,
    pub authority: &'info Pubkey,
}

impl<'info> WrappedAccount<'info> for PermissionAccount<'info> {
    type Target = Permission;
    type Args = PermissionArgs<'info>;

    fn new(_info: &'info AccountInfo) -> Result<Self, ProgramError> {
        panic!("not supported")
    }

    fn new_with_args(info: &'info AccountInfo, args: Self::Args) -> Result<Self, ProgramError> {
        Ok(Self {
            info,
            inner: Permission::load_and_check(info, args.controller, args.authority)?,
        })
    }

    fn info(&self) -> &'info AccountInfo {
        self.info
    }

    fn inner(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ReserveAccount<'info> {
    info: &'info AccountInfo,
    inner: Reserve,
}

pub struct ReserveArgs<'info> {
    pub controller: &'info Pubkey,
}

impl<'info> WrappedAccount<'info> for ReserveAccount<'info> {
    type Target = Reserve;
    type Args = ReserveArgs<'info>;

    fn new(_info: &'info AccountInfo) -> Result<Self, ProgramError> {
        panic!("not supported")
    }

    fn new_with_args(info: &'info AccountInfo, args: Self::Args) -> Result<Self, ProgramError> {
        Ok(Self {
            info,
            inner: Reserve::load_and_check(info, args.controller)?,
        })
    }

    fn info(&self) -> &'info AccountInfo {
        self.info
    }

    fn inner(&self) -> &Self::Target {
        &self.inner
    }

    fn key(&self) -> &'info Pubkey {
        self.info().key()
    }
}

impl<'info> ReserveAccount<'info> {
    pub fn new_with_args_mut(info: &'info AccountInfo, args: ReserveArgs) -> Result<Self, ProgramError> {
        Ok(Self {
            info,
            inner: Reserve::load_and_check_mut(info, args.controller)?,
        })
    }


    pub fn inner(&mut self) -> &mut Reserve {
         &mut self.inner
    }
}