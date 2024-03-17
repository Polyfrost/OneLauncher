use serde::{Deserialize, Serialize};

use super::JavaVersion;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub main_class: String,
    pub java_version: JavaVersion,
}