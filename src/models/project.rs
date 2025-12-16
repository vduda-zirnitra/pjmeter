use sqlx::types::chrono::{DateTime, Local, TimeZone};

pub struct Project {
    pub label: String,
    pub absolute_path: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Project {
    pub fn get_created_at(&self) -> DateTime<Local> {
        Local.timestamp_opt(self.created_at, 0).unwrap()
    }

    pub fn get_updated_at(&self) -> DateTime<Local> {
        Local.timestamp_opt(self.updated_at, 0).unwrap()
    }
}
