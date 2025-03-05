#[repr(u8)]
pub enum PermissionStatus {
    Suspended = 0,
    Active = 1,
}


impl PermissionStatus {
    pub fn max() -> u8 {
        PermissionStatus::Active as u8
    }
}

impl From<u8> for PermissionStatus {
    fn from(byte: u8) -> PermissionStatus {
        match byte {
            0 => PermissionStatus::Suspended,
            1 => PermissionStatus::Active,
            _ => panic!("Invalid u8 for PermissionStatus"),
        }
    }
}

impl From<PermissionStatus> for u8 {
    fn from(status: PermissionStatus) -> u8 {
        status as u8
    }
}