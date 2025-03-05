#[repr(u8)]
pub enum IntegrationType {
    Undefined = 0,
    SplTokenVault = 1,
    SplTokenExternal = 2,
    YieldBearingVault = 3,
    SplLimitOrder = 4,
    CctpBridge = 5,
    LayerZeroBridge = 6,
    // NovaDex
    // Kamino
    // Drift
    // MarginFi
    // Save/Solend
}


impl IntegrationType {
    pub fn max() -> u8 {
        IntegrationType::LayerZeroBridge as u8
    }
}

impl From<u8> for IntegrationType {
    fn from(byte: u8) -> IntegrationType {
        match byte {
            0 => IntegrationType::Undefined,
            1 => IntegrationType::SplTokenVault,
            2 => IntegrationType::SplTokenExternal,
            3 => IntegrationType::YieldBearingVault,
            4 => IntegrationType::SplLimitOrder,
            5 => IntegrationType::CctpBridge,
            6 => IntegrationType::LayerZeroBridge,
            _ => panic!("Invalid u8 for IntegrationType"),
        }
    }
}

impl From<IntegrationType> for u8 {
    fn from(integration_type: IntegrationType) -> u8 {
        integration_type as u8
    }
}