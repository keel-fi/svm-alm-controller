use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::pubkey::Pubkey;
use shank::ShankType;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq, ShankType)]
pub struct KaminoConfig {
    /// The Kamino `Market`.
    pub market: Pubkey,
    /// The Kamino `Reserve`, linked to `reserve_liquidity_mint`.
    pub reserve: Pubkey,
    /// The `Reserve` farm collateral.
    /// This `Pubkey` can be derived, but some `Reserves` may not have a farm_collateral (in that case it defaults to `Pubkey::default()`).
    pub reserve_farm_collateral: Pubkey,
    /// The `Reserve` farm debt.
    /// This `Pubkey` can be derived, but some `Reserves` may not have a farm_debt (in that case it defaults to `Pubkey::default()`).
    pub reserve_farm_debt: Pubkey,
    /// The reserve liquidity mint. This is the mint that is deposited (lent) into the Kamino `Reserve`.
    pub reserve_liquidity_mint: Pubkey,
    /// The obligation, different kamino integrations can share a single obligation.
    /// 
    /// An `Obligation` is an account from the KLEND program used to track deposits/borrows into/from a Kamino market.
    /// It contains 8 slots for deposits (`[ObligationCollateral; 8]`), where each slot stores information such as 
    /// the `deposit_reserve` (e.g. USDC) and the `deposited_amount` (in terms of the minted LP tokens, called collateral in KLEND).
    /// 
    /// For more details, see: https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/obligation.rs
    pub obligation: Pubkey,
    /// The `Obligation` id. Since an `Obligation` has 8 slots for deposits, it can be necessary to create a new `Obligation`
    /// for a certain market if all the slots are being used. The id is passed as an argument in initialization, and is used
    /// to derive the obligation PDA.
    pub obligation_id: u8,
    /// Padding
    pub _padding: [u8; 30]
}