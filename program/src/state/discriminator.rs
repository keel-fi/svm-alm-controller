extern crate alloc;

use alloc::vec::Vec;

pub trait Discriminator {
    const DISCRIMINATOR: u8;
}

#[repr(u8)]
pub enum AccountDiscriminators {
    UninitializedDiscriminator = 0,
    ControllerDiscriminator = 1,
    PermissionDiscriminator = 2,
    IntegrationDiscriminator = 3,
    ReserveDiscriminator = 4,
    SwapPair = 5,
    Oracle = 6,
}

pub trait AccountSerialize: Discriminator {
    /// Serialize the struct with the Discriminator prepended.
    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        // Discriminator
        data.push(Self::DISCRIMINATOR);
        data.extend(self.to_bytes_inner());

        data
    }

    fn to_bytes_inner(&self) -> Vec<u8>;
}
