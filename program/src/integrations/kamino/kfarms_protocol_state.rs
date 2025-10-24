use account_zerocopy_deserialize::AccountZerocopyDeserialize;
use bytemuck::{Pod, Zeroable};
use pinocchio::pubkey::Pubkey;

use crate::constants::anchor_discriminator;

// --------- State copied from kfarms program ---------
// Note: we make slight modifications such as changing
// enums and bools to u8 in order to be Pod.

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct FarmTokenInfo {
    pub mint: Pubkey,
    pub decimals: u64,
    pub token_program: Pubkey,
    pub _padding: [u64; 6],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct RewardInfo {
    pub token: FarmTokenInfo,
    pub rewards_vault: Pubkey,
    pub rewards_available: u64,
    pub reward_schedule_curve: RewardScheduleCurve,
    pub min_claim_duration_seconds: u64,
    pub last_issuance_ts: u64,
    pub rewards_issued_unclaimed: u64,
    pub rewards_issued_cumulative: u64,
    pub reward_per_share_scaled: u128,
    pub placeholder_0: u64,
    pub reward_type: u8,
    pub rewards_per_second_decimals: u8,
    pub _padding_0: [u8; 6],
    pub _padding_1: [u64; 20],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct RewardScheduleCurve {
    pub points: [RewardPerTimeUnitPoint; 20],
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct RewardPerTimeUnitPoint {
    pub ts_start: u64,
    pub reward_per_time_unit: u64,
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct FarmState {
    pub farm_admin: Pubkey,
    pub global_config: Pubkey,
    pub token: FarmTokenInfo,
    pub reward_infos: [RewardInfo; 10],
    pub num_reward_tokens: u64,
    pub num_users: u64,
    pub total_staked_amount: u64,
    pub farm_vault: Pubkey,
    pub farm_vaults_authority: Pubkey,
    pub farm_vaults_authority_bump: u64,
    pub delegate_authority: Pubkey,
    pub time_unit: u8,
    pub is_farm_frozen: u8,
    pub is_farm_delegated: u8,
    pub _padding_0: [u8; 5],
    pub withdraw_authority: Pubkey,
    pub deposit_warmup_period: u32,
    pub withdrawal_cooldown_period: u32,
    pub total_active_stake_scaled: u128,
    pub total_pending_stake_scaled: u128,
    pub total_pending_amount: u64,
    pub slashed_amount_current: u64,
    pub slashed_amount_cumulative: u64,
    pub slashed_amount_spill_address: Pubkey,
    pub locking_mode: u64,
    pub locking_start_timestamp: u64,
    pub locking_duration: u64,
    pub locking_early_withdrawal_penalty_bps: u64,
    pub deposit_cap_amount: u64,
    pub scope_prices: Pubkey,
    pub scope_oracle_price_id: u64,
    pub scope_oracle_max_age: u64,
    pub pending_farm_admin: Pubkey,
    pub strategy_id: Pubkey,
    pub delegated_rps_admin: Pubkey,
    pub vault_id: Pubkey,
    pub second_delegated_authority: Pubkey,
    // padding expanded into 3 chunks to be Pod (length 74)
    pub _padding_1: [u64; 32],
    pub _padding_2: [u64; 32],
    pub _padding_3: [u64; 10],
}

impl AccountZerocopyDeserialize<8> for FarmState {
    const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "FarmState");
}

impl FarmState {
    pub fn find_reward_index_and_rewards_available(
        &self,
        reward_mint: &Pubkey,
        reward_token_program: &Pubkey,
    ) -> Option<(u64, u64)> {
        self.reward_infos
            .iter()
            .enumerate()
            .find_map(|(index, reward_info)| {
                if &reward_info.token.mint == reward_mint
                    && &reward_info.token.token_program == reward_token_program
                {
                    Some((index as u64, reward_info.rewards_available))
                } else {
                    None
                }
            })
    }
}
