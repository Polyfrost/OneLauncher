use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum BisectExit {
    None = 0,
    Clean = 1,
    Crashed = 2,
}

impl BisectExit {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn from_repr(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Clean),
            2 => Some(Self::Crashed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ClusterBisectSessionRow {
    pub cluster_id: i64,
    pub round: i64,
    pub pending_exit: i64,
    pub started_at: String,
    pub retry_note: Option<String>,
}

impl ClusterBisectSessionRow {
    pub fn pending_exit(&self) -> BisectExit {
        BisectExit::from_repr(self.pending_exit).unwrap_or(BisectExit::None)
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ClusterBisectModRow {
    pub cluster_id: i64,
    pub hash: String,
    pub cleared_round: Option<i64>,
    pub testing: i64,
}
