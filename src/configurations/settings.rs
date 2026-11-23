use super::config::{default_stay, default_browser};
use serde::Deserialize;

/// Shared settings: `[configurations]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub home_url: String,
    pub email: String,

    #[serde(default = "default_browser")]
    pub browser: String,

    #[serde(default = "default_stay")]
    pub stay_minutes: u64,
}
