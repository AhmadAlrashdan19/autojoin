use chromiumoxide::cdp::browser_protocol::network::{CookieParam, TimeSinceEpoch};
use chromiumoxide::{Browser, BrowserConfig, Page};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use futures::StreamExt;

use crate::configurations::config::{Config, home_dir};
use crate::configurations::browser::resolve_browser;
use crate::website::has_button;
use crate::alarm::alarm_start;
use crate::teams::run;

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn profile_dir(browser: &str) -> Result<PathBuf> {
    let browser_name = Path::new(browser)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(browser);
    Ok(home_dir()?.join(format!(".config/autojoin/profile-{browser_name}")))
}

async fn persist_msal_key(page: &Page) {
    let Ok(cookies) = page.get_cookies().await else {
        return;
    };
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        + 90.0 * 86400.0;

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
    let set_default = args.iter().any(|a| a == "--set-as-default");
    let login_only = args.iter().any(|a| a == "--login");
    let selector = arg_value(&args, "--meeting");
    let browser_arg = arg_value(&args, "--browser");

    let cfg = Config::load()?;
    let settings = &cfg.settings;

    if set_default {
        let browser_name = browser_arg
            .ok_or_else(|| anyhow!(r#"Error: "--browser <name>" is required with --set-as-default"#))?;
        Config::set_browser_as_default(browser_name)?;
        println!(
            "Setting default browser successfully => browser: {}",
            browser_name
        );
        return Ok(());
    }

    let browser_name = browser_arg.unwrap_or(settings.browser.as_str());

    let meeting = if login_only {
        None
    } else {
        Some(cfg.pick(selector)?)
    };

    let browser_path = resolve_browser(browser_name)?;

    let browser_cfg = BrowserConfig::builder()
        .chrome_executable(&browser_path)
        .user_data_dir(profile_dir(browser_name)?)
        .with_head()
        .window_size(1280, 800)
        .viewport(None)
        .arg("--deny-permission-prompts")
        .arg("--lang=en-US")
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
        let alarm = alarm_start(&format!("Auto-join failed: {e}. Join manually NOW!"));

        // Wait for the browser to be closed. If the user joins by hand: silence
        // the alarm and keep the browser open for the stay time.
        let stay = meeting.stay_minutes.unwrap_or(settings.stay_minutes);
        let mut deadline = Instant::now() + Duration::from_secs(600);
        let mut manual = false;
        while Instant::now() < deadline {
            if tokio::time::timeout(Duration::from_secs(2), browser.wait())
                .await
                .is_ok()
            {
                break; // browser closed by the user
            }
            if !manual && has_button(&page, "Leave|Hang up", false).await {
                manual = true;
                alarm.stop();
                eprintln!("Manual join detected, alarm stopped. Browser stays open {stay} min.");
                deadline = Instant::now() + Duration::from_secs(stay * 60);
            }
        }
        alarm.stop();
    }

    persist_msal_key(&page).await;
    let _ = browser.close().await;
    let _ = browser.wait().await;

    Ok(())
}
