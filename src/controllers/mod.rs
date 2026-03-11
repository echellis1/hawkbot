use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControllerType {
    Daktronics,
}

impl ControllerType {
    pub fn compatible_sports(self) -> &'static [ActiveSport] {
        match self {
            ControllerType::Daktronics => ActiveSport::ALL,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActiveSport {
    Baseball,
    Basketball,
    Football,
    Soccer,
    Volleyball,
    Wrestling,
    WaterPolo,
}

impl ActiveSport {
    pub const ALL: &'static [Self] = &[
        Self::Baseball,
        Self::Basketball,
        Self::Football,
        Self::Soccer,
        Self::Volleyball,
        Self::Wrestling,
        Self::WaterPolo,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Baseball => "baseball",
            Self::Basketball => "basketball",
            Self::Football => "football",
            Self::Soccer => "soccer",
            Self::Volleyball => "volleyball",
            Self::Wrestling => "wrestling",
            Self::WaterPolo => "water_polo",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daktronics_supports_configured_sports() {
        assert!(ControllerType::Daktronics
            .compatible_sports()
            .contains(&ActiveSport::Basketball));
    }
}
