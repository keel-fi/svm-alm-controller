use shank::ShankType;

pub trait Discriminator {
    const DISCRIMINATOR: u8;
}

#[derive(ShankType)]
#[repr(u8)]
pub enum AccountDiscriminators {
    UninitializedDiscriminator = 0,
    ControllerDiscriminator = 1,
    PermissionDiscriminator = 2,
    IntegrationDiscriminator = 3,
    ReserveDiscriminator = 4,
    OracleDiscriminator = 5,
}
