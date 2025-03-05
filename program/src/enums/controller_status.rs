#[repr(u8)]
pub enum ControllerStatus {
    Suspended = 0,
    Active = 1,
}


impl ControllerStatus {
    pub fn max() -> u8 {
        ControllerStatus::Active as u8
    }
}

impl From<u8> for ControllerStatus {
    fn from(byte: u8) -> ControllerStatus {
        match byte {
            0 => ControllerStatus::Suspended,
            1 => ControllerStatus::Active,
            _ => panic!("Invalid u8 for ControllerStatus"),
        }
    }
}

impl From<ControllerStatus> for u8 {
    fn from(status: ControllerStatus) -> u8 {
        status as u8
    }
}