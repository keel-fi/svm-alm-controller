#[repr(u8)]
pub enum IntegrationStatus {
    Suspended = 0,
    Active = 1,
}


impl IntegrationStatus {
    pub fn max() -> u8 {
        IntegrationStatus::Active as u8
    }
}

impl From<u8> for IntegrationStatus {
    fn from(byte: u8) -> IntegrationStatus {
        match byte {
            0 => IntegrationStatus::Suspended,
            1 => IntegrationStatus::Active,
            _ => panic!("Invalid u8 for IntegrationStatus"),
        }
    }
}

impl From<IntegrationStatus> for u8 {
    fn from(status: IntegrationStatus) -> u8 {
        status as u8
    }
}