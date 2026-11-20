use anyhow::{Context, Ok, Result, bail};
use serde::Deserialize;
use std::{env, fs, path::PathBuf};

fn default_stay() -> u64 {
    90
}

fn default_browser() -> String {
    "/usr/bin/brave".into()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub home_url: String,
    pub link: String,
    pub email: String,

    #[serde(default = "default_browser")]
    pub browser: String,

    #[serde(default = "default_stay")]
    pub stay_minutes: u64,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    configurations: Config,
}

impl Config {
    fn path() -> PathBuf {
        let home = env::var("HOME").expect("HOME not set");
        PathBuf::from(home).join(
            /*".config/teams-autojoin/config.toml"*/
            "Desktop/Studying/Github_Repos/Projects/teams-autojoin/config.toml",
        )
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        let text =
            fs::read_to_string(&path).with_context(|| format!("Cannot read {}", path.display()))?;
        let file: ConfigFile =
            toml::from_str(&text).with_context(|| format!("Invalid TOML in {}", path.display()))?;
        let cfg = file.configurations;

        if !cfg.link.starts_with("https://") || !cfg.home_url.starts_with("https://") {
            bail!("Config: Link must start with https://");
        }

        if !cfg.email.contains('@') {
            bail!("Config: Email looks invalid");
        }

        Ok(cfg)
    }
}
