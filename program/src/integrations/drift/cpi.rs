use crate::cpi_instruction;
use crate::{constants::anchor_discriminator, integrations::drift::constants::DRIFT_PROGRAM_ID};

cpi_instruction! {
    /// Initialize Drift UserStats account.
    /// This only needs to be called ONCE per Controller.
    /// NOTE: check for existence before invoking.
    pub struct InitializeUserStats<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "initialize_user_stats"),
        accounts: {
            user_stats: Writable,
            state: Writable,
            authority: Signer,
            payer: Writable<Signer>,
            rent: Readonly,
            system_program: Readonly
        }
    }
}

cpi_instruction! {
    /// Updates a Drift user pool_id.
    /// Note: called when the pool_id of the user being initialized
    /// is != 0.
    pub struct UpdateUserPoolId<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "update_user_pool_id"),
        accounts: {
            user: Writable,
            authority: Signer,
        },
        args: {
            sub_account_id: u16,
            pool_id: u8,
        }
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
        accounts: {
            user: Writable,
            user_stats: Writable,
            state: Writable,
            authority: Signer,
            payer: Writable<Signer>,
            rent: Readonly,
            system_program: Readonly
        },
        args: {
            sub_account_id: u16,
            name: [u8; 32]
        }
    }
}

cpi_instruction! {
    /// Deposit tokens into a Drift spot market
    pub struct Deposit<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "deposit"),
        accounts: {
            state: Readonly,
            user: Writable,
            user_stats: Writable,
            authority: Signer,
            spot_market_vault: Writable,
            user_token_account: Writable,
            token_program: Readonly
        },
        remaining_accounts: remaining_accounts,
        args: {
            market_index: u16,
            amount: u64,
            reduce_only: bool
        }
    }
}

cpi_instruction! {
    /// Withdraw tokens from a Drift spot market
    pub struct Withdraw<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "withdraw"),
        accounts: {
            state: Readonly,
            user: Writable,
            user_stats: Writable,
            authority: Signer,
            spot_market_vault: Writable,
            drift_signer: Readonly,
            user_token_account: Writable,
            token_program: Readonly,
        },
        remaining_accounts: remaining_accounts,
        args: {
            market_index: u16,
            amount: u64,
            reduce_only: bool
        }
    }
}

cpi_instruction! {
    /// Update a SpotMarket to have up to date interest
    pub struct UpdateSpotMarketCumulativeInterest<'info> {
        program: DRIFT_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "update_spot_market_cumulative_interest"),
        accounts: {
            state: Readonly,
            spot_market: Writable,
            oracle: Readonly,
            spot_market_vault: Readonly,
        }
    }
}
