use std::time::{Duration, Instant};
use anyhow::{Result, bail};
use chromiumoxide::Page;
use tokio::time::sleep;

use crate::website::{click_button, find_button, has_button, type_into};
use crate::configurations::{meeting::Meeting, settings::Settings};
use crate::snapshot;

const SIGNED_IN: &str = "Join now|Continue on this browser|Use the web app instead|^Chat$|^Calendar$";

async fn wait_ready(page: &Page, timeout_s: u64) -> Result<bool> {
    for _ in 0..(timeout_s * 2) {
        if has_button(page, SIGNED_IN, false).await {
            return Ok(true);
        }
        
        if has_button(page, "^Sign in$", false).await {
            bail!("Session expired. Run: autojoin --login");
        }
        sleep(Duration::from_millis(500)).await;
    }
    Ok(false)
}

pub async fn is_guest_page(page: &Page) -> bool {
    page.url()
        .await
        .ok()
        .flatten()
        .map(|u| u.contains("light-meetings"))
        .unwrap_or(false)
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

async fn warmup(page: &Page, cfg: &Settings) -> Result<()> {
    for attempt in 1..=3u32 {
        // Warm up the signed-in session first.
        let now = Instant::now();
        match page.goto(&cfg.home_url).await {
            Err(e) => eprintln!("Warmup goto failed ({attempt}): {e}"),
            Ok(_) => {
                let ready = wait_ready(page, 40).await?;
                eprintln!("Warmup took: {}s", now.elapsed().as_secs_f32());

                if ready {
                    return Ok(());
                }
                snapshot::snapshot(page, "warmup").await;
            }
        }
        if attempt < 3 {
            sleep(Duration::from_secs(5u64 << (attempt - 1))).await;
        }
    }
    bail!("Warmup failed after 3 attempts");
}

pub async fn run(page: &Page, cfg: &Settings, meeting: &Meeting) -> Result<()> {
    let stay_minutes = meeting.stay_minutes.unwrap_or(cfg.stay_minutes);
    warmup(page, cfg).await?;

    for attempt in 1..=3 {
        if let Err(e) = page.goto(&meeting.link).await {
            eprintln!("Error: Cannot go to the link. {e:#?}");
            sleep(Duration::from_secs(5u64 << (attempt - 1))).await;
            continue;
        }

        snapshot::snapshot(page, "after goto").await;

        // Popups appear late: keep clearing them until "Join now" is enabled.
        let mut sign_in_tried = false;
        let mut joined = false;
        for _ in 0..120 {
            // Already in the meeting (e.g. joined by hand)? Stop clicking.
            if has_button(page, "Leave|Hang up", false).await {
                joined = true;
                break;
            }
            
            if !sign_in_tried && is_guest_page(page).await {
                sign_in_tried = true;
                if let Err(e) = sign_in(page, &cfg.email).await {
                    eprintln!("sign-in failed: {e:#}");
                }
            }
            
            let _ = has_button(page, "Continue on this browser|Use the web app instead", true).await;
            let _ = has_button(page, "Continue without audio or video", true).await;

            if has_button(page, "Join now", true).await {
                // Join counts only if a Leave button shows up afterwards.
                joined = find_button(page, "Leave|Hang up", 30, false).await.is_ok();
                break;
            }
            sleep(Duration::from_millis(500)).await;
        }

        if joined {
            println!("joined {}, staying {} minutes", meeting.name, stay_minutes);
            sleep(Duration::from_secs(stay_minutes * 60)).await;
            return Ok(());
        }

        snapshot::screenshot(page, &format!("attempt-{attempt}-failed")).await;
        if let Ok(Some(url)) = page.url().await {
            eprintln!(
                "attempt {attempt} failed, url = {}",
                url.split(['?', '#']).next().unwrap_or("")
            );
        } else {
            eprintln!("attempt {attempt} failed, URL unavailable");
        }
    }
    bail!("Could not join after 3 attempts")
}
