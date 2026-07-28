use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::{Display, FromRepr};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize_repr,
    Deserialize_repr,
    FromRepr,
    Display,
    Default,
)]
#[repr(i64)]
pub enum ClusterStage {
    #[default]
    NotReady = 0,
    Downloading = 1,
    Repairing = 2,
    Ready = 3,
}

impl ClusterStage {
    pub fn is_busy(self) -> bool {
        matches!(self, Self::Downloading | Self::Repairing)
    }
}
