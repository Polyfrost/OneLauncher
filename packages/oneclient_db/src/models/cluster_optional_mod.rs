use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum OptionalModStatus {
    New = 0,
    Skipped = 1,
}

impl OptionalModStatus {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn from_repr(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::New),
            1 => Some(Self::Skipped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ClusterOptionalModRow {
    pub id: i64,
    pub cluster_id: i64,
    pub bundle_name: String,
    pub package_id: String,
    pub bundle_version_id: String,
    pub seen_status: i64,
    pub queued_at: String,
}

impl ClusterOptionalModRow {
    pub fn status(&self) -> OptionalModStatus {
        OptionalModStatus::from_repr(self.seen_status).unwrap_or(OptionalModStatus::New)
    }
}
