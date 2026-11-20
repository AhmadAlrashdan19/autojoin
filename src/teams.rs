use anyhow::{Result, bail};
use chromiumoxide::Page;
use std::time::Duration;
use tokio::time::sleep;

use crate::browser::{click_button, find_button, type_into};
use crate::config::Config;
use crate::snapshot;

pub async fn is_guest_page(page: &Page) -> bool {
    page.url()
        .await
        .ok()
        .flatten()
        .map(|u| u.contains("light-meetings"))
        .unwrap_or(false)
}

/// Close optional popups. Each step is allowed to fail silently.
pub async fn dismiss_popups(page: &Page) {
    let _ = click_button(page, "Continue on this browser|Use the web app instead", 4).await;
    let _ = click_button(page, "Continue without audio or video", 4).await;
}

/// Guest page: click Sign in, type email, click Next.
pub async fn sign_in(page: &Page, email: &str) -> Result<()> {
    click_button(page, "^Sign in$", 8).await?;
    type_into(page, "input[placeholder='Enter your email']", email, 10).await?;
    click_button(page, "^Next$", 10).await?; // waits until Next is enabled
    sleep(Duration::from_secs(8)).await;
    snapshot::snapshot(page, "after next").await;
    Ok(())
}

pub async fn run(page: &Page, cfg: &Config) -> Result<()> {
    // Warm up the signed-in session first.
    page.goto(&cfg.home_url).await?;
    let _ = find_button(page, "Join now|Continue on this browser|Use the web app instead|^Sign in$|^Retry$", 40, false).await;
    snapshot::snapshot(page, "warmup").await;

    for attempt in 1..=3 {
        page.goto(&cfg.link).await?;
        sleep(Duration::from_secs(6)).await;
        dismiss_popups(page).await;
        snapshot::snapshot(page, "after goto").await;

        if is_guest_page(page).await {
            if let Err(e) = sign_in(page, &cfg.email).await {
                eprintln!("sign-in failed: {e:#}");
            }
            dismiss_popups(page).await;
        }
        snapshot::snapshot(page, "before join").await;

        // Join counts only if a Leave button shows up afterwards.
        let joined = click_button(page, "Join now", 25).await.is_ok()
            && find_button(page, "Leave|Hang up", 30, false).await.is_ok();

        if joined {
            eprintln!("joined, staying {} minutes", cfg.stay_minutes);
            sleep(Duration::from_secs(cfg.stay_minutes * 60)).await;
            return Ok(());
        }

        snapshot::screenshot(page, &format!("attempt-{attempt}-failed")).await;
        eprintln!("attempt {attempt} failed, url = {:?}", page.url().await?);
    }
    bail!("Could not join after 3 attempts")
}
