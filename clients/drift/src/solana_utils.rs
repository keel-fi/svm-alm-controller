#[cfg(feature = "program")]
pub use pinocchio_pubkey::Pubkey;
#[cfg(not(feature = "program"))]
pub use solana_sdk::pubkey::Pubkey;

#[cfg(feature = "program")]
pub use pinocchio::pubkey;
#[cfg(not(feature = "program"))]
pub use solana_sdk::pubkey;

#[cfg(feature = "program")]
pub use pinocchio::program_error::ProgramError;
#[cfg(not(feature = "program"))]
pub use solana_sdk::program_error::ProgramError;

#[cfg(feature = "program")]
pub fn try_find_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Option<(Pubkey, u8)> {
    pinocchio::pubkey::find_program_address(seeds, program_id)
        .map(|(pubkey, bump)| (pubkey, bump as u8))
}

#[cfg(not(feature = "program"))]
pub fn try_find_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Option<(Pubkey, u8)> {
    solana_sdk::pubkey::Pubkey::try_find_program_address(seeds, program_id)
}
