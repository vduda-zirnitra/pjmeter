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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_get_created_at() {
        let project = Project {
            label: "test".to_string(),
            absolute_path: "/test/path".to_string(),
            created_at: 1700000000, // Nov 14, 2023 22:13:20 UTC
            updated_at: 1700000000,
        };

        let created_at = project.get_created_at();
        assert_eq!(created_at.timestamp(), 1700000000);
    }

    #[test]
    fn test_project_get_updated_at() {
        let project = Project {
            label: "test".to_string(),
            absolute_path: "/test/path".to_string(),
            created_at: 1700000000,
            updated_at: 1700100000, // ~27 hours later
        };

        let updated_at = project.get_updated_at();
        assert_eq!(updated_at.timestamp(), 1700100000);
    }

    #[test]
    fn test_project_timestamps_different() {
        let project = Project {
            label: "test".to_string(),
            absolute_path: "/test/path".to_string(),
            created_at: 1700000000,
            updated_at: 1700100000,
        };

        let created_at = project.get_created_at();
        let updated_at = project.get_updated_at();
        assert!(updated_at > created_at);
    }
}
