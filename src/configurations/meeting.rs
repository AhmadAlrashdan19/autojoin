use anyhow::{Context, Result, anyhow};
use chrono::{NaiveTime, Weekday};
use serde::Deserialize;

/// One `[[meetings]]` entry.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meeting {
    pub name: String,
    pub link: String,
    pub days: Vec<String>,
    pub time: String,
    pub stay_minutes: Option<u64>,
}

impl Meeting {
    pub fn schedule(&self) -> Result<(Vec<Weekday>, NaiveTime)> {
        let days = self.days
            .iter()
            .map(|d| {
                d.parse::<Weekday>()
                .map_err(|_| anyhow!("Meeting '{}': Bad dat: {d} (use Mon, Tue, ...)", self.name))
            })
            .collect::<Result<Vec<_>>>()?;

        let time = NaiveTime::parse_from_str(&self.time, "%H:%M").with_context(|| {
            format!("Meeting '{}': time {} must be HH:MM", self.name, self.time)
        })?;

        Ok((days, time))
    }
}
