use borsh::BorshDeserialize;
use pinocchio::program_error::ProgramError;

#[derive(Debug, Default, PartialEq, BorshDeserialize)]
pub struct OftSendParams {
    pub dst_eid: u32,
    pub to: [u8; 32],
    pub amount_ld: u64,
    pub min_amount_ld: u64,
    // pub options: Vec<u8>,              // <- Verify empty? TBD
    // pub compose_msg: Option<Vec<u8>>,  // <- Verify this is empty
    // pub native_fee: u64,               // <- No risk
    // pub lz_token_fee: u64,             // <- No risk
}

impl OftSendParams {
    const DISCRIMINATOR: [u8; 8] = [102, 251, 20, 187, 65, 75, 12, 69];

    const TRUNCATED_LEN: usize = 4 + 32 + 8 + 8;

    pub fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        if data[0..8] != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Self::try_from_slice(&data[9..Self::TRUNCATED_LEN + 8])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}
