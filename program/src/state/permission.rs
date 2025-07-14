use super::{
    discriminator::{AccountDiscriminators, Discriminator},
    nova_account::NovaAccount,
};
use crate::{
    constants::PERMISSION_SEED, enums::PermissionStatus, processor::shared::create_pda_account,
};
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{try_find_program_address, Pubkey},
    sysvars::{rent::Rent, Sysvar},
};
use shank::ShankAccount;

/// Account that tracks the permisisons of a given Address for a specific Controller.
#[derive(Clone, Debug, PartialEq, ShankAccount, Copy, BorshSerialize, BorshDeserialize)]
#[repr(C)]
pub struct Permission {
    /// Controller this Permission applies to
    pub controller: Pubkey,
    /// Address that has the power to use the enabled permissions
    pub authority: Pubkey,
    /// Status of the Permissions (i.e. active or suspended)
    pub status: PermissionStatus,
    /// Enables the Permission's authority to create or modify other Permissions
    pub can_manage_permissions: bool,
    /// Enables the Permission's authority to execute ("Push") SplTokenExternal transfers,
    /// sending tokens to a wallet external from the Controller
    pub can_invoke_external_transfer: bool,
    /// Enables the Permission's authority to execute ("Push") AtomicSwaps, swapping
    /// one of the Controllers Reserve tokens to another token in a separate Reserve.
    pub can_execute_swap: bool,
    /// Enables the Permission's authority to execute ("Push" AND "Pull") SplTokenSwap integrations,
    /// adding or removing liquidity from a SPL Token Swap pool.
    pub can_reallocate: bool,
    /// Enables the Permission's authority to freeze the Controller, preventing any
    /// "Push" or "Pull" type actions from being invoked.
    pub can_freeze: bool,
    /// Enables the Permission's authority to unfreeze the Controller.
    pub can_unfreeze: bool,
    /// Enables the Permission's authority to update any Integration's status, LUT, and rate limit params.
    pub can_manage_integrations: bool,
    /// Enables the Permission's authority to suspend any Permission, EXCEPT for
    /// a Super Permission with `can_manage_permissions` enabled.
    pub can_suspend_permissions: bool,
    pub _padding: [u8; 31],
}

impl Discriminator for Permission {
    const DISCRIMINATOR: u8 = AccountDiscriminators::PermissionDiscriminator as u8;
}

impl NovaAccount for Permission {
    const LEN: usize = 65 + 7 + 32; 

    fn derive_pda(&self) -> Result<(Pubkey, u8), ProgramError> {
        try_find_program_address(
            &[
                PERMISSION_SEED,
                self.controller.as_ref(),
                self.authority.as_ref(),
            ],
            &crate::ID,
        )
        .ok_or(ProgramError::InvalidSeeds)
    }
}

impl Permission {
    pub fn check_data(&self, controller: &Pubkey, authority: &Pubkey) -> Result<(), ProgramError> {
        if self.authority.ne(authority) || self.controller.ne(controller) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    pub fn load_and_check(
        account_info: &AccountInfo,
        controller: &Pubkey,
        authority: &Pubkey,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        // Check PDA

        let permission: Self = NovaAccount::deserialize(&account_info.try_borrow_data()?).unwrap();
        permission.check_data(controller, authority)?;
        permission.verify_pda(account_info)?;
        Ok(permission)
    }

    pub fn load_and_check_mut(
        account_info: &AccountInfo,
        controller: &Pubkey,
        authority: &Pubkey,
    ) -> Result<Self, ProgramError> {
        // Ensure account owner is the program
        if !account_info.is_owned_by(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }
        let permission: Self =
            NovaAccount::deserialize(&account_info.try_borrow_mut_data()?).unwrap();
        permission.check_data(controller, authority)?;
        permission.verify_pda(account_info)?;
        Ok(permission)
    }

    pub fn init_account(
        account_info: &AccountInfo,
        payer_info: &AccountInfo,
        controller: Pubkey,
        authority: Pubkey,
        status: PermissionStatus,
        can_manage_permissions: bool,
        can_invoke_external_transfer: bool,
        can_execute_swap: bool,
        can_reallocate: bool,
        can_freeze: bool,
        can_unfreeze: bool,
        can_manage_integrations: bool,
        can_suspend_permissions: bool,
    ) -> Result<Self, ProgramError> {
        // Create and serialize the controller
        let permission = Permission {
            controller,
            authority,
            status,
            can_manage_permissions,
            can_invoke_external_transfer,
            can_execute_swap,
            can_reallocate,
            can_freeze,
            can_unfreeze,
            can_manage_integrations,
            can_suspend_permissions,
            _padding: [0; 31],
        };

        // Derive the PDA
        let (pda, bump) = permission.derive_pda()?;
        if account_info.key().ne(&pda) {
            return Err(ProgramError::InvalidSeeds.into()); // PDA was invalid
        }

        // Account creation PDA
        let rent = Rent::get()?;
        let bump_seed = [bump];
        let signer_seeds = [
            Seed::from(PERMISSION_SEED),
            Seed::from(&controller),
            Seed::from(&authority),
            Seed::from(&bump_seed),
        ];
        create_pda_account(
            payer_info,
            &rent,
            1 + Self::LEN,
            &crate::ID,
            account_info,
            signer_seeds,
        )?;

        // Commit the account on-chain
        permission.save(account_info)?;

        Ok(permission)
    }

    pub fn update_and_save(
        &mut self,
        status: Option<PermissionStatus>,
        can_manage_permissions: Option<bool>,
        can_invoke_external_transfer: Option<bool>,
        can_execute_swap: Option<bool>,
        can_reallocate: Option<bool>,
        can_freeze: Option<bool>,
        can_unfreeze: Option<bool>,
        can_manage_integrations: Option<bool>,
        can_suspend_permissions: Option<bool>,
    ) -> Result<(), ProgramError> {
        if let Some(status) = status {
            self.status = status;
        }
        if let Some(can_manage_permissions) = can_manage_permissions {
            self.can_manage_permissions = can_manage_permissions;
        }
        if let Some(can_suspend_permissions) = can_suspend_permissions {
            self.can_suspend_permissions = can_suspend_permissions;
        }
        if let Some(can_invoke_external_transfer) = can_invoke_external_transfer {
            self.can_invoke_external_transfer = can_invoke_external_transfer;
        }
        if let Some(can_execute_swap) = can_execute_swap {
            self.can_execute_swap = can_execute_swap;
        }
        if let Some(can_reallocate) = can_reallocate {
            self.can_reallocate = can_reallocate;
        }
        if let Some(can_freeze) = can_freeze {
            self.can_freeze = can_freeze;
        }
        if let Some(can_unfreeze) = can_unfreeze {
            self.can_unfreeze = can_unfreeze;
        }
        if let Some(can_manage_integrations) = can_manage_integrations {
            self.can_manage_integrations = can_manage_integrations;
        }

        Ok(())
    }

    pub fn can_manage_permissions(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_manage_permissions
    }

    pub fn can_suspend_permissions(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_suspend_permissions
    }

    pub fn can_manage_integrations(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_manage_integrations
    }

    pub fn can_execute_swap(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_execute_swap
    }

    pub fn can_reallocate(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_reallocate
    }

    pub fn can_invoke_external_transfer(&self) -> bool {
        self.status == PermissionStatus::Active && self.can_invoke_external_transfer
    }
}
