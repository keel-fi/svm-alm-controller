extern crate alloc;

use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
};
use pinocchio_log::log;

use pythnet_sdk::{
    accumulators::merkle::MerkleRoot,
    hashers::keccak256_160::Keccak160,
    messages::Message,
    pythnet::WORMHOLE_PID,
    wire::{
        from_slice,
        v1::{MerklePriceUpdate, WormholeMessage, WormholePayload},
    },
};

use solana_program::{
    clock::Clock, keccak, program_memory::sol_memcpy, secp256k1_recover::secp256k1_recover,
    sysvar::Sysvar,
};
use wormhole_raw_vaas::{GuardianSetSig, Vaa};

/// Represents a data source for Pyth price updates
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct DataSource {
    /// The chain ID of the emitter
    pub emitter_chain_id: u16,
    /// The address of the emitter on the source chain
    pub emitter_address: Pubkey,
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
#[repr(u8)]
pub enum PythTransformation {
    None,
    UsePrice,
    UseEmaPrice,
    InvertInput,
}

const MAX_PYTH_TRANSFORMATIONS: usize = 3;

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct PythConfig {
    /// FeedId of the Pyth price oracle
    pub feed_id: [u8; 32],
    /// Minimum number of Guardian signatures required for update
    pub min_signatures: u8,
    /// Maximum staleness for a pyth price feed update.
    pub max_staleness_seconds: u32,
    /// Guardian set
    pub guardian_set: Pubkey,
    /// Transformation logic
    pub transformations: [PythTransformation; MAX_PYTH_TRANSFORMATIONS],
}

/// The data returned from the validation process
/// on a Pyth Proof received
pub struct ValidatedPythUpdate {
    /// Pyth Feed Id
    pub feed_id: [u8; 32],
    /// The address of the relevant wormhole guardian set
    pub guardian_set: Pubkey,
    /// The number of verified signatures present in the proof
    pub num_signatures: u8,
    /// The price
    pub price: i64,
    /// The EMA price
    pub ema_price: i64,
    /// Confidence metric
    pub conf: u64,
    /// Exponent of the price data
    pub exponent: i32,
    /// The time the price benchmark relates to
    pub publish_time: i64,
    /// The chain ID of the emitter
    pub emitter_chain: u16,
    /// The address of the emitter on the source chain
    pub emitter_address: [u8; 32],
}

impl PythConfig {
    pub fn transform(&self, pyth_update: ValidatedPythUpdate) -> Result<(), ProgramError> {
        let mut current_numerator: Option<u64> = None;
        let mut current_denominator: Option<u64> = None;

        for transformation in &self.transformations {
            // Get the base price value from the Pyth update
            match transformation {
                PythTransformation::None => {}
                PythTransformation::UsePrice => {
                    let denominator = 10u64.pow(-pyth_update.exponent as u32);
                    let numerator = pyth_update.price as u64;
                    current_numerator = Some(numerator);
                    current_denominator = Some(denominator);
                }
                PythTransformation::UseEmaPrice => {
                    let denominator = 10u64.pow(-pyth_update.exponent as u32);
                    let numerator = pyth_update.ema_price as u64;
                    current_numerator = Some(numerator);
                    current_denominator = Some(denominator);
                }
                PythTransformation::InvertInput => {
                    if current_numerator.is_none() || current_denominator.is_none() {
                        return Err(ProgramError::InvalidAccountData.into());
                    }
                    // Flip the numerator and denominator
                    let old_denominator = current_denominator.unwrap();
                    let old_numerator = current_numerator.unwrap();
                    current_numerator = Some(old_denominator);
                    current_denominator = Some(old_numerator);
                }
            }
        }

        if current_numerator.is_none() || current_denominator.is_none() {
            return Err(ProgramError::InvalidAccountData.into());
        }

        // TODO: Return a uniformed update
        Ok(())
    }

    pub fn validate_update(
        &self,
        guardian_set_account_info: &AccountInfo,
        vaa: Vec<u8>,
        merkle_price_update: MerklePriceUpdate,
    ) -> Result<ValidatedPythUpdate, ProgramError> {
        let guardian_set = deserialize_guardian_set_checked(
            &guardian_set_account_info,
            &Pubkey::from(WORMHOLE_PID),
        );
        if guardian_set.is_err() {
            log!("Failed to deserialize guardian set");
            return Err(ProgramError::InvalidAccountData.into());
        }
        let guardian_set = guardian_set.unwrap();

        let vaa = Vaa::parse(&vaa);
        if vaa.is_err() {
            log!("Failed to parse VAA");
            return Err(ProgramError::InvalidAccountData.into());
        };
        let vaa = vaa.unwrap();

        if vaa.version() != 1 {
            log!("Invalid VAA version");
            return Err(ProgramError::InvalidAccountData.into());
        }

        let guardian_set = guardian_set;
        if vaa.guardian_set_index() != guardian_set.index {
            log!("Guardian set index mismatch");
            return Err(ProgramError::InvalidAccountData.into());
        }

        // Extract emitter chain and address from VAA body
        let emitter_chain = vaa.body().emitter_chain();
        let emitter_address = vaa.body().emitter_address();

        let guardian_keys = &guardian_set.keys;
        let digest = keccak::hash(keccak::hash(vaa.body().as_ref()).as_ref());
        let mut last_guardian_index = None;
        for sig in vaa.signatures() {
            let index = usize::from(sig.guardian_index());
            if let Some(last_index) = last_guardian_index {
                if index <= last_index {
                    log!("Non-increasing guardian signature indices");
                    return Err(ProgramError::InvalidAccountData.into());
                }
            }
            let guardian_pubkey = guardian_keys.get(index).ok_or_else(|| {
                log!("Invalid guardian index");
                ProgramError::InvalidAccountData
            })?;
            verify_guardian_signature(&sig, guardian_pubkey, digest.as_ref())?;
            last_guardian_index = Some(index);
        }

        let wormhole_message =
            WormholeMessage::try_from_bytes(vaa.payload().as_ref()).map_err(|_| {
                log!("Invalid Wormhole message");
                ProgramError::InvalidAccountData
            })?;

        let root: MerkleRoot<Keccak160> = MerkleRoot::new(match wormhole_message.payload {
            WormholePayload::Merkle(merkle_root) => merkle_root.root,
        });

        if !root.check(
            merkle_price_update.proof.clone(),
            &merkle_price_update.message.as_ref(),
        ) {
            log!("Invalid price update");
            return Err(ProgramError::InvalidAccountData.into());
        }

        let message = from_slice::<byteorder::BE, Message>(merkle_price_update.message.as_ref())
            .map_err(|_| {
                log!("Failed to deserialize message");
                ProgramError::InvalidAccountData
            })?;

        match message {
            Message::PriceFeedMessage(price_feed_message) => Ok(ValidatedPythUpdate {
                feed_id: price_feed_message.feed_id,
                num_signatures: vaa.signature_count(),
                price: price_feed_message.price,
                ema_price: price_feed_message.ema_price,
                conf: price_feed_message.conf,
                exponent: price_feed_message.exponent,
                publish_time: price_feed_message.publish_time,
                guardian_set: *guardian_set_account_info.key(),
                emitter_chain,
                emitter_address,
            }),
            _ => {
                log!("Unsupported Wormhole message type");
                Err(ProgramError::InvalidAccountData.into())
            }
        }
    }
}

fn deserialize_guardian_set_checked(
    account_info: &AccountInfo,
    wormhole: &Pubkey,
) -> Result<GuardianSetData, ProgramError> {
    let mut guardian_set_data: &[u8] = &account_info.try_borrow_data()?;
    if !account_info.is_owned_by(wormhole) {
        log!("GuardianSet account not owned by wormhole program");
        return Err(ProgramError::InvalidAccountData.into());
    }
    let guardian_set = GuardianSetData::try_deserialize(&mut guardian_set_data)?;
    let expected_address = find_program_address(
        &[
            GUARDIAN_SET_SEED_PREFIX,
            guardian_set.index.to_be_bytes().as_ref(),
        ],
        wormhole,
    )
    .0;
    if expected_address != *account_info.key() {
        log!("Invalid Guardian Set PDA");
        return Err(ProgramError::InvalidAccountData.into());
    }
    let timestamp = Clock::get()
        .map_err(|_| ProgramError::UnsupportedSysvar)?
        .unix_timestamp as u32;
    if guardian_set.expiration_time != 0 && !guardian_set.is_active(&timestamp) {
        log!("Guardian Set expired");
        return Err(ProgramError::InvalidAccountData.into());
    }
    Ok(guardian_set)
}

fn verify_guardian_signature(
    sig: &GuardianSetSig,
    guardian_pubkey: &[u8; 20],
    digest: &[u8],
) -> Result<(), ProgramError> {
    let recovered = {
        let pubkey = secp256k1_recover(digest, sig.recovery_id(), &sig.rs()).map_err(|_| {
            log!("Invalid signature");
            ProgramError::InvalidInstructionData
        })?;
        let hashed = keccak::hash(&pubkey.to_bytes());
        let mut eth_pubkey = [0; 20];
        sol_memcpy(&mut eth_pubkey, &hashed.0[12..], 20);
        eth_pubkey
    };
    if recovered != *guardian_pubkey {
        log!("Invalid Guardian key recovery");
        return Err(ProgramError::InvalidAccountData.into());
    }
    Ok(())
}

/// Represents a public key in the GuardianSet
pub type GuardianPublicKey = [u8; 20];

/// GuardianSetData represents the data structure for the guardian set.
pub struct GuardianSetData {
    /// Index representing an incrementing version number for this guardian set.
    pub index: u32,
    /// ETH style public keys
    pub keys: Vec<GuardianPublicKey>,
    /// Timestamp representing the time this guardian became active.
    pub creation_time: u32,
    /// Expiration time when VAAs issued by this set are no longer valid.
    pub expiration_time: u32,
}

const GUARDIAN_SET_SEED_PREFIX: &[u8] = b"GuardianSet";

impl GuardianSetData {
    /// Manually deserialize GuardianSetData from a byte slice.
    pub fn try_deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        let mut offset = 8;

        // Deserialize index
        let index = u32::from_le_bytes(data[offset..offset + 4].try_into().map_err(|_| {
            log!("Failed to deserialize index");
            ProgramError::InvalidAccountData
        })?);
        offset += 4;

        // Deserialize number of keys
        let num_keys = u32::from_le_bytes(data[offset..offset + 4].try_into().map_err(|_| {
            log!("Failed to deserialize number of keys");
            ProgramError::InvalidAccountData
        })?);
        offset += 4;

        // Deserialize keys
        let mut keys = Vec::with_capacity(num_keys as usize);
        for _ in 0..num_keys {
            let key = data[offset..offset + 20].try_into().map_err(|_| {
                log!("Failed to deserialize guardian key");
                ProgramError::InvalidAccountData
            })?;
            keys.push(key);
            offset += 20;
        }

        // Deserialize creation_time
        let creation_time =
            u32::from_le_bytes(data[offset..offset + 4].try_into().map_err(|_| {
                log!("Failed to deserialize creation time");
                ProgramError::InvalidAccountData
            })?);
        offset += 4;

        // Deserialize expiration_time
        let expiration_time =
            u32::from_le_bytes(data[offset..offset + 4].try_into().map_err(|_| {
                log!("Failed to deserialize expiration time");
                ProgramError::InvalidAccountData
            })?);

        Ok(GuardianSetData {
            index,
            keys,
            creation_time,
            expiration_time,
        })
    }

    /// Number of guardians in the set
    pub fn num_guardians(&self) -> u8 {
        self.keys.iter().filter(|v| **v != [0u8; 20]).count() as u8
    }

    /// Check if the guardian set is active based on the current timestamp
    pub fn is_active(&self, current_time: &u32) -> bool {
        *current_time >= self.creation_time && *current_time < self.expiration_time
    }
}
