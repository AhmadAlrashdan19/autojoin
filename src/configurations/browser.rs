use anyhow::{Result, anyhow, bail};
use which;

pub fn resolve_browser(name: &str) -> Result<String> {
    if name.contains('/') {
        return Ok(name.to_string());
    }

    let candidates: &[&str] = match name {
        "brave"    => &["brave", "brave-browser"],
        "helium"   => &["helium", "helium-browser"],
        "chrome"   => &["google-chrome", "google-chrome-stable"],
        "chromium" => &["chromium", "chromium-browser"],
        "edge"     => &["microsoft-edge", "microsoft-edge-stable"],
        "vivaldi"  => &["vivaldi", "vivaldi-stable"],
        other      => bail!("Unknown browser '{other}'. Use a known name or a full path"),
    };

    candidates
        .iter()
        .find_map(|c| which::which(c).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| anyhow!("Browser '{name}' not found in PATH"))
}