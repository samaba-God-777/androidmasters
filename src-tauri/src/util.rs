//! Small shared helpers.

/// Current local time as ISO-8601 with seconds (stored as TEXT in SQLite).
pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}
