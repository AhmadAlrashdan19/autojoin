use chromiumoxide::cdp::browser_protocol::network::{CookieParam, TimeSinceEpoch};
use chromiumoxide::{Browser, BrowserConfig, Page};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Result, anyhow};
use futures::StreamExt;
use std::path::PathBuf;

use crate::configurations::config::{Config, home_dir};
use crate::alarm::alarm;
use crate::teams::run;

fn profile_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".config/autojoin/profile-brave"))
}

async fn persist_msal_key(page: &Page) {
    let Ok(cookies) = page.get_cookies().await else { return };
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64() + 90.0 * 86400.0;

    let params: Vec<CookieParam> = cookies
        .iter()
        .filter(|c| c.name == "msal.cache.encryption")
        .filter_map(|c| {
            CookieParam::builder()
                .name(&c.name)
                .value(&c.value)
                .domain(&c.domain)
                .path(&c.path)
                .secure(c.secure)
                .http_only(c.http_only)
                .expires(TimeSinceEpoch::new(exp))
                .build()
                .ok()
        })
        .collect();
    if !params.is_empty() {
        let _ = page.set_cookies(params).await;
    }
}

pub async fn bootstrap() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let login_only = std::env::args().any(|a| a == "--login");
    let cfg = Config::load()?;
    let settings = &cfg.settings;
    let selector = args.iter()
        .position(|a| a == "--meeting")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str);

    let meeting = if login_only {
        None
    } else {
        Some(cfg.pick(selector)?)
    };

    let browser_cfg = BrowserConfig::builder()
        .chrome_executable(&settings.browser)
        .user_data_dir(profile_dir()?)
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

    let Some(meeting) = meeting else {
        let page = browser.new_page(settings.home_url.as_str()).await?;
        println!("Sign in fully (including MFA), then press Enter here...");
        let mut s = String::new();
        std::io::stdin().read_line(&mut s)?;
        persist_msal_key(&page).await;
        browser.close().await?;
        browser.wait().await?;
        return Ok(());
    };

    let page = browser.new_page("about:blank").await?;
    if let Err(e) = run(&page, settings, meeting).await {
        eprintln!("ERROR: {e:#}");
        persist_msal_key(&page).await;
        alarm(&format!("Auto-join failed: {e}. Join manually NOW!")).await;
        browser.wait().await?;
    }
    persist_msal_key(&page).await;
    let _ = browser.close().await;
    let _ = browser.wait().await;

    Ok(())
}
