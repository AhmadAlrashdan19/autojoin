use anyhow::{Context, Result, anyhow, bail};
use chrono::{Datelike, Local};
use serde::Deserialize;
use std::{env, fs, path::PathBuf};

use super::meeting::Meeting;
use super::settings::Settings;

/// A meeting is picked when "now" is within this many minutes of its start time.
const PICK_WINDOW_MIN: i64 = 20;

pub fn default_stay() -> u64 { 90 }

pub fn default_browser() -> String { "/usr/bin/brave".into() }

pub fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME not set")
}

pub struct Config {
    pub settings: Settings,
    meetings: Vec<Meeting>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    configurations: Settings,
    #[serde(default)]
    meetings: Vec<Meeting>,
}

impl Config {
    fn path() -> Result<PathBuf> {
        if let Some(p) = env::var_os("AUTOJOIN_CONFIG") {
            return Ok(PathBuf::from(p));
        }

        Ok(home_dir()?.join("Desktop/Studying/Github_Repos/Projects/autojoin/config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        let text =
            fs::read_to_string(&path).with_context(|| format!("Cannot read {}", path.display()))?;
        let file: ConfigFile =
            toml::from_str(&text).with_context(|| format!("Invalid TOML in {}", path.display()))?;
        let cfg = &file.configurations;

        if !cfg.home_url.starts_with("https://") {
            bail!("Config: Home URL must start with https://");
        }

        if !cfg.email.contains('@') {
            bail!("Config: Email looks invalid");
        }

        let mut seen: Vec<String> = Vec::new();
        for meeting in &file.meetings {
            if meeting.name.trim().is_empty() {
                bail!("Config: A meeting has an empty name");
            }

            let key = meeting.name.to_lowercase();
            if seen.contains(&key) {
                bail!("Config: Duplicate meeting name '{}'", meeting.name);
            }
            seen.push(key);

            if !meeting.link.starts_with("https://") {
                bail!("Config: Meeting '{}' link must start with https://", meeting.name);
            }

            if meeting.days.is_empty() {
                bail!("Config: Meeting '{}' days is empty", meeting.name);
            }
        }

        Ok(Config {
            settings: file.configurations,
            meetings: file.meetings
        })
    }

    pub fn pick(&self, selector: Option<&str>) -> Result<&Meeting> {
        let names = || {
            self.meetings
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };

        if self.meetings.is_empty() {
            bail!("Config: No [[meetings]] defined");
        }

        if let Some(name) = selector {
            return self
            .meetings
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow!("No meeting named '{name}'. Known: {}", names()));
        }

        let now = Local::now();
        let today = now.weekday();
        let mut best: Option<(&Meeting, i64)> = None;

        for meeting in &self.meetings {
            let (days, time) = meeting.schedule()?;

            if !days.contains(&today) {
                continue;
            }

            let diff = (now.time() - time).num_minutes().abs();
            if diff <= PICK_WINDOW_MIN && best.map_or(true, |(_, d)| diff < d) {
                best = Some((meeting, diff));
            }
        }

        best.map(|(m, _)| m).ok_or_else(|| {
            anyhow!(
                "No meeting scheduled near {} on {today}. Known: {}. Use --meeting <name>",
                now.format("%H:%M"),
                names()
            )
        })
    }
}
