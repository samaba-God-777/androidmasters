//! Result item shared by diagnostics (USB now; FRP and reset preflight later).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Info,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckItem {
    pub key: &'static str,
    pub title: String,
    pub status: CheckStatus,
    /// What was actually detected.
    pub detail: String,
    /// What to do about it (empty when nothing is needed).
    pub recommendation: String,
}

impl CheckItem {
    pub fn new(key: &'static str, title: &str, status: CheckStatus, detail: impl Into<String>) -> Self {
        Self { key, title: title.to_string(), status, detail: detail.into(), recommendation: String::new() }
    }
    pub fn advise(mut self, recommendation: impl Into<String>) -> Self {
        self.recommendation = recommendation.into();
        self
    }
}
