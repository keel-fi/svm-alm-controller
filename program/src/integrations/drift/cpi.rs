use crate::cpi_instruction;
use crate::{constants::anchor_discriminator, integrations::drift::constants::DRIFT_PROGRAM_ID};

cpi_instruction! {
    /// Initialize Drift UserStats account.
    /// This only needs to be called ONCE per Controller.
    /// NOTE: check for existence before invoking.
    pub struct InitializeUserStats<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "initialize_user_stats"),
        
        user_stats: Writable,
        state: Writable,
        authority: Signer,
        payer: Writable<Signer>,
        rent: Readonly,
        system_program: Readonly
    }
}

cpi_instruction! {
    /// Initialize Drift User account.
    /// This must be called per subaccount.
    /// NOTE: the Name on the User will simply be the Subaccount ID since
    /// we do not require a human readable Name.
    pub struct InitializeUser<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "initialize_user"),
        
        user: Writable,
        user_stats: Writable,
        state: Writable,
        authority: Signer,
        payer: Writable<Signer>,
        rent: Readonly,
        system_program: Readonly;
        
        sub_account_id: u16,
        name: [u8; 32]
    }
}
