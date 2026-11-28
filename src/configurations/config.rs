use anyhow::{Context, Ok, Result, anyhow, bail};
use chrono::{Datelike, Local};
use serde::Deserialize;
use std::{env, fs, path::PathBuf};
use toml_edit::{DocumentMut, value};

use super::browser::resolve_browser;
use super::meeting::Meeting;
use super::settings::Settings;

/// A meeting is picked when "now" is within this many minutes of its start time.
const PICK_WINDOW_MIN: i64 = 20;

pub fn default_stay() -> u64 { 90 }

pub fn default_browser() -> String { "brave".into() }

pub fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME not set")
}

pub fn set_browser_in_toml(content: &str, browser: &str) -> Result<String> {
    let mut doc = content
        .parse::<DocumentMut>()
        .context("This TOML content is invalid")?;
    let item = doc
        .get_mut("configurations")
        .ok_or_else(|| anyhow!("'configurations' table does not exist"))?;
    let table = item
        .as_table_mut()
        .ok_or_else(|| anyhow!("'configurations' must be a [configurations] table"))?;
    table["browser"] = value(browser);
    Ok(doc.to_string())
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

        Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        let text = fs::read_to_string(&path).with_context(|| {
            format!(
                "Cannot read {}. Copy config.example.toml to config.toml",
                path.display()
            )
        })?;
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
                bail!(
                    "Config: Meeting '{}' link must start with https://",
                    meeting.name
                );
            }

            if meeting.days.is_empty() {
                bail!("Config: Meeting '{}' days is empty", meeting.name);
            }

            meeting.schedule()?;
        }

        Ok(Config {
            settings: file.configurations,
            meetings: file.meetings,
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
            if diff <= PICK_WINDOW_MIN && best.is_none_or(|(_, d)| diff < d) {
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

    pub fn set_browser_as_default(browser: &str) -> Result<()> {
        let _ = resolve_browser(browser)?;
        let path = Self::path()?;
        let text =
            fs::read_to_string(&path).with_context(|| format!("Cannot read {}", path.display()))?;
        let new_config_file_content = set_browser_in_toml(&text, browser)?;

        let tmp_path = path.with_file_name("config-tmp.toml");
        fs::write(&tmp_path, &new_config_file_content)
            .with_context(|| format!("Cannot write in the tmp file {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &path)
            .with_context(|| format!("Cannot rename the tmp file {}", tmp_path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurations_is_not_a_table() {
        let content = r#"configurations = { home_url = "https://team.net" }"#;
        let err = set_browser_in_toml(content, "helium").unwrap_err();
        assert!(err.to_string().contains("must be a"));
    }

    #[test]
    fn key_is_exist() {
        let content = r#"
            [configurations]
            home_url = "https://team.net"
            email = "user@comp.met"
            browser = "brave"   # brave | chrome | chromium | edge | vivaldi | /full/path
            stay_minutes = 60
"#;
        let doc = set_browser_in_toml(content, "helium");
        assert_eq!(
            doc.unwrap(),
            r#"
            [configurations]
            home_url = "https://team.net"
            email = "user@comp.met"
            browser = "helium"
            stay_minutes = 60
"#
        );
    }

    #[test]
    fn key_does_not_exist() {
        let content = r#"
            [configurations]
            home_url = "https://team.net"
            email = "user@comp.met"
            #  browser = "brave"   # brave | chrome | chromium | edge | vivaldi | /full/path
            stay_minutes = 60

            [[meetings]]
            name = "math"
            link = "https://team.net"
            days = ["Mon", "Wed"]
            time = "17:25"

"#;
        let doc = set_browser_in_toml(content, "helium").unwrap();
        println!("{doc}");
        assert!(doc.contains(r#"browser = "helium""#));
        assert!(doc.contains(r#"# brave | chrome | chromium | edge | vivaldi | /full/path"#));
        assert!(doc.contains(r#"stay_minutes = 60"#));
        assert!(doc.contains(
            r#"browser = "helium"

            [[meetings]]"#
        ));
    }

    #[test]
    fn table_does_not_exist() {
        let content = r#"
            [[meetings]]
            name = "math"
            link = "https://team.net"
            days = ["Mon", "Wed"]
            time = "17:25"
"#;
        let doc = set_browser_in_toml(content, "helium");
        assert!(doc.is_err());
    }
}
