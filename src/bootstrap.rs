use anyhow::{Result, anyhow};
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;
use std::{path::PathBuf, time::Duration};
use tokio::time::sleep;

use crate::alarm::alarm;
use crate::config::Config;
use crate::teams::run;

fn profile_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join("./config/teams-autojoin/profile-brave")
}

pub async fn bootstrap() -> Result<()> {
    let login_only = std::env::args().any(|a| a == "--login");
    let cfg = Config::load()?;

    let browser_cfg = BrowserConfig::builder()
        .chrome_executable(&cfg.browser)
        .user_data_dir(profile_dir())
        .with_head()
        .window_size(1280, 800)
        .viewport(None)
        .arg("--use-fake-ui-for-media-stream")
        .build()
        .map_err(|e| anyhow!(e))?;

    let (mut browser, mut handler) = Browser::launch(browser_cfg).await?;
    tokio::spawn(async move {
        while let Some(_event) = handler.next().await {
            // ignore non-fatal errors, keep loop alive
        }
    });

    if login_only {
        let _page = browser.new_page(cfg.home_url.as_str()).await?;
        println!("Sign in fully (including MFA), then press Enter here...");
        let mut s = String::new();
        std::io::stdin().read_line(&mut s)?;
        browser.close().await?;
        return Ok(());
    }

    let page = browser.new_page("about:blank").await?;
    if let Err(e) = run(&page, &cfg).await {
        eprintln!("ERROR: {e:#}");
        alarm(&format!("Auto-join failed: {e}. Join manually NOW!"));
        sleep(Duration::from_secs(60 * 60)).await;
    }
    browser.close().await?;

    Ok(())
}
