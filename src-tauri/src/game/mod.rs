use serde::{Deserialize, Serialize};

pub mod client;
pub mod manifest;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JavaVersion {
    V8,
    V16,
    V17
}

impl ToString for JavaVersion {
    fn to_string(&self) -> String {
        match self {
            JavaVersion::V8 => "8".to_string(),
            JavaVersion::V16 => "16".to_string(),
            JavaVersion::V17 => "17".to_string()
        }
    }
}