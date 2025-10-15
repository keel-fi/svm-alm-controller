#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};
use litesvm::LiteSVM;
use solana_sdk::{program_error::ProgramError, pubkey::Pubkey};
use svm_alm_controller::constants::anchor_discriminator;

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

impl FarmState {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "FarmState");

    pub fn find_reward_index_and_rewards_available(
        &self, 
        reward_mint: &Pubkey,
        reward_token_program: &Pubkey
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


#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct GlobalConfig {
    pub global_admin: Pubkey,
    pub treasury_fee_bps: u64,
    pub treasury_vaults_authority: Pubkey,
    pub treasury_vaults_authority_bump: u64,
    pub pending_global_admin: Pubkey,
    // padding expanded into 4 chunks to be Pod (length 126)
    pub _padding_1: [u128; 32],
    pub _padding_2: [u128; 32],
    pub _padding_3: [u128; 32],
    pub _padding_4: [u128; 30],
}

impl GlobalConfig {
    pub const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "GlobalConfig");

    pub fn try_from(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }
}

#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct UserState {
    pub user_id: u64,
    pub farm_state: Pubkey,
    pub owner: Pubkey,
    pub is_farm_delegated: u8,
    pub _padding_0: [u8; 7],
    pub rewards_tally_scaled: [u128; 10],
    pub rewards_issued_unclaimed: [u64; 10],
    pub last_claim_ts: [u64; 10],
    pub active_stake_scaled: u128,
    pub pending_deposit_stake_scaled: u128,
    pub pending_deposit_stake_ts: u64,
    pub pending_withdrawal_unstake_scaled: u128,
    pub pending_withdrawal_unstake_ts: u64,
    pub bump: u64,
    pub delegatee: Pubkey,
    pub last_stake_ts: u64,
    // padding expanded into 2 chunks to be Pod (length 50)
    pub _padding_1: [u64; 32],
    pub _padding_2: [u64; 18],
}

impl UserState {
    const DISCRIMINATOR: [u8; 8] = anchor_discriminator("account", "UserState");

    pub fn try_from(data: &[u8]) -> Result<&Self, ProgramError> {
        if data[..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        bytemuck::try_from_bytes(&data[8..]).map_err(|_| ProgramError::InvalidAccountData)
    }
    
    pub fn get_rewards(
        svm: &LiteSVM, 
        user_state_pk: &Pubkey, 
        global_config_pk: &Pubkey, 
        reward_index: usize
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let user_state_acc = svm.get_account(user_state_pk)
            .expect("Failed to get UserState");
        let user_state = Self::try_from(&user_state_acc.data)?;

        let reward = user_state.rewards_issued_unclaimed[reward_index];
        if reward == 0 {
            return Ok(0)
        }

        let global_config_acc = svm.get_account(global_config_pk)
            .expect("Failed to get GlobalConfig");

        let global_config_state = GlobalConfig::try_from(&global_config_acc.data)?;
        let reward_treasury = Self::u64_mul_div(reward, global_config_state.treasury_fee_bps, 10000)?;
        let reward_user = reward
            .checked_sub(reward_treasury)
            .ok_or_else(|| ProgramError::ArithmeticOverflow)?;

        Ok(reward_user)
    }

    fn u64_mul_div(a: u64, b: u64, c: u64) -> Result<u64, ProgramError> {
        let a: u128 = a.into();
        let b: u128 = b.into();
        let c: u128 = c.into();

        let numerator = a
            .checked_mul(b)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        let result = numerator
            .checked_div(c)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        result
            .try_into()
            .map_err(|_| ProgramError::ArithmeticOverflow)
    }
}