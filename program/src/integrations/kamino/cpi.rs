use pinocchio::pubkey::Pubkey;

use crate::{
    constants::anchor_discriminator,
    cpi_instruction,
    integrations::kamino::constants::{KAMINO_FARMS_PROGRAM_ID, KAMINO_LEND_PROGRAM_ID},
};

cpi_instruction! {
    /// Initializes a Kamino `Obligation`.
    /// The tag is used for determining the type of obligation,
    /// which are used for frontend differentiation, we default to tag 0 (`VanillaObligation`).
    /// An obligation has 8 slots for deposits and 5 slots for borrows.
    pub struct InitializeObligation<'info> {
        program: KAMINO_LEND_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "init_obligation"),
        accounts: {
            obligation_owner: Signer,
            payer: Writable<Signer>,
            obligation: Writable,
            lending_market: Readonly,
            /// For a `VanillaObligation`, `system_program` should be passed.
            seed_1: Readonly,
            /// For a `VanillaObligation`, `system_program` should be passed.
            seed_2: Readonly,
            owner_user_metadata: Readonly,
            rent: Readonly,
            system_program: Readonly
        },
        args: {
            tag: u8,
            id: u8,
        }
    }
}

cpi_instruction! {
    /// Initialize a Kamino `UserMetadata` account.
    /// This only needs to be called ONCE per controller.
    pub struct InitializeUserMetadata<'info> {
        program: KAMINO_LEND_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "init_user_metadata"),
        accounts: {
            owner: Signer,
            payer: Writable<Signer>,
            user_metadata: Writable,
            referrer_user_metadata: Readonly,
            rent: Readonly,
            system_program: Readonly
        },
        args: {
            user_lookup_table: Pubkey,
        }
    }
}

cpi_instruction! {
    /// Initialize an Obligation `Farm`, linked to a `reserve.collateral_farm` (mode 0)
    /// or a `reserve.debt_farm` (mode 1).
    /// Obligation farms are used for rewards harvesting.
    pub struct InitializeObligationFarmForReserve<'info> {
        program: KAMINO_LEND_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "init_obligation_farms_for_reserve"),
        accounts: {
            payer: Writable<Signer>,
            owner: Readonly,
            obligation: Writable,
            market_authority: Readonly,
            kamino_reserve: Writable,
            reserve_farm_state: Writable,
            obligation_farm: Writable,
            lending_market: Readonly,
            farms_program: Readonly,
            rent: Readonly,
            system_program: Readonly
        },
        args: {
            mode: u8
        }
    }
}

cpi_instruction! {
    /// Deposits liquidity into a kamino `Reserve`,
    /// increasing the obligation collateral.
    pub struct DepositReserveLiquidityAndObligationCollateralV2<'info> {
        program: KAMINO_LEND_PROGRAM_ID,
        discriminator: anchor_discriminator(
            "global",
            "deposit_reserve_liquidity_and_obligation_collateral_v2",
        ),
        accounts: {
            owner: Writable<Signer>,
            obligation: Writable,
            lending_market: Readonly,
            market_authority: Readonly,
            kamino_reserve: Writable,
            reserve_liquidity_mint: Readonly,
            reserve_liquidity_supply: Writable,
            reserve_collateral_mint: Writable,
            reserve_collateral_supply: Writable,
            user_source_liquidity: Writable,
            /// Placeholder account, should be used with `KLEND` program pubkey
            placeholder_user_destination_collateral: Readonly,
            collateral_token_program: Readonly,
            liquidity_token_program: Readonly,
            instruction_sysvar: Readonly,
            obligation_farm_user_state: Writable,
            reserve_farm_state: Writable,
            farms_program: Readonly
        },
        args: {
            liquidity_amount: u64,
        }
    }
}

cpi_instruction! {
    /// Withdraws collateral from an `Obligation`.
    pub struct WithdrawObligationCollateralAndRedeemReserveCollateralV2<'info> {
        program: KAMINO_LEND_PROGRAM_ID,
        discriminator: anchor_discriminator(
            "global",
            "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2",
        ),
        accounts: {
            owner: Writable<Signer>,
            obligation: Writable,
            lending_market: Readonly,
            market_authority: Readonly,
            kamino_reserve: Writable,
            reserve_liquidity_mint: Readonly,
            reserve_collateral_supply: Writable,
            reserve_collateral_mint: Writable,
            reserve_liquidity_supply: Writable,
            user_liquidity_destination: Writable,
            /// Placeholder account, should be used with `KLEND` program pubkey
            placeholder_user_destination_collateral: Readonly,
            collateral_token_program: Readonly,
            liquidity_token_program: Readonly,
            instruction_sysvar: Readonly,
            obligation_farm_user_state: Writable,
            reserve_farm_state: Writable,
            farms_program: Readonly
        },
        args: {
            collateral_amount: u64,
        }
    }
}

cpi_instruction! {
    /// Harvests earned rewards. It should be called if the
    /// `farm_state.rewards_available` > 0
    pub struct HarvestReward<'info> {
        program: KAMINO_FARMS_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "harvest_reward"),
        accounts: {
            owner: Writable<Signer>,
            user_state: Writable,
            farm_state: Writable,
            global_config: Readonly,
            reward_mint: Readonly,
            user_reward_ata: Writable,
            rewards_vault: Writable,
            rewards_treasury_vault: Writable,
            farm_vaults_authority: Readonly,
            scope_prices: Readonly,
            token_program: Readonly
        },
        args: {
            reward_index: u64,
        }
    }
}

cpi_instruction! {
    /// Refreshes an `Obligation`.
    /// Note: To be used after initialization, since it is assumed
    /// that the `Obligation` has no associated deposits and borrows,
    /// which would need to be passed as remaining accounts.
    pub struct RefreshObligationAfterInit<'info> {
        program: KAMINO_LEND_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "refresh_obligation"),
        accounts: {
            lending_market: Readonly,
            obligation: Writable,
        }
    }
}
