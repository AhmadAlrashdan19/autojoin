use anyhow::{anyhow, bail, Result};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use std::{path::PathBuf, process::Command, time::Duration};
use tokio::time::sleep;

const LINK: &str = "https://teams.cloud.microsoft/v2/?meetingjoin=true#/meet/367538546402368?p=qB7CVKuFFGGjclr2fX";
const HOME_URL: &str = "https://teams.cloud.microsoft";
const EMAIL: &str = "2025971007@";
const BRAVE: &str = "/usr/bin/brave";
const STAY_MINUTES: u64 = 90;

fn dirs_home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME not set"))
}

fn alarm(msg: &str) {
    let _ = Command::new("notify-send")
        .args(["-u", "critical", "Teams", msg])
        .status();
    let _ = Command::new("paplay")
        .arg("/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga")
        .status();
}

/// Print URL, visible buttons and inputs. Debug aid.
async fn snapshot(page: &Page, tag: &str) {
    let js = r#"JSON.stringify({
        url: location.href.slice(0, 120),
        buttons: [...document.querySelectorAll('button,[role=button],a')]
            .map(x => (x.innerText || x.getAttribute('aria-label') || '').trim())
            .filter(Boolean).slice(0, 25),
        inputs: [...document.querySelectorAll('input')]
            .map(x => x.type + ':' + (x.placeholder || x.name || ''))
    })"#;
    match page.evaluate(js).await {
        Ok(v) => eprintln!("[{tag}] {}", v.into_value::<String>().unwrap_or_default()),
        Err(e) => eprintln!("[{tag}] evaluate failed: {e}"),
    }
}

/// Wait for an ENABLED button matching the regex. Click it if `click` is true.
async fn find_button(page: &Page, pattern: &str, timeout_s: u64, click: bool) -> Result<()> {
    let js = format!(
        r#"(() => {{
            const re = new RegExp("{pattern}", "i");
            const b = [...document.querySelectorAll('button, [role=button], a')]
                .find(x => !x.disabled && x.getAttribute('aria-disabled') !== 'true'
                    && re.test((x.innerText || x.getAttribute('aria-label') || '').trim()));
            if (!b) return false;
            if ({click}) b.click();
            return true;
        }})()"#
    );

    for _ in 0..(timeout_s * 2) {
        let found = match page.evaluate(js.as_str()).await {
            Ok(v) => v.into_value::<bool>().unwrap_or(false),
            Err(_) => false, // page navigating, retry
        };
        if found {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    bail!("Button /{pattern}/ not found or disabled");
}

async fn click_button(page: &Page, pattern: &str, timeout_s: u64) -> Result<()> {
    find_button(page, pattern, timeout_s, true).await
}

/// Wait for an input, click it, type text.
async fn type_into(page: &Page, selector: &str, text: &str, timeout_s: u64) -> Result<()> {
    for _ in 0..(timeout_s * 2) {
        if let Ok(el) = page.find_element(selector).await {
            el.click().await?;
            el.type_str(text).await?;
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    bail!("Input {selector} not found");
}

async fn is_guest_page(page: &Page) -> bool {
    page.url()
        .await
        .ok()
        .flatten()
        .map(|u| u.contains("light-meetings"))
        .unwrap_or(false)
}

/// Close optional popups. Each step is allowed to fail silently.
async fn dismiss_popups(page: &Page) {
    let _ = click_button(page, "Continue on this browser|Use the web app instead", 4).await;
    let _ = click_button(page, "Continue without audio or video", 4).await;
}

/// Guest page: click Sign in, type email, click Next.
async fn sign_in(page: &Page) -> Result<()> {
    click_button(page, "^Sign in$", 8).await?;
    type_into(page, "input[placeholder='Enter your email']", EMAIL, 10).await?;
    click_button(page, "^Next$", 10).await?; // waits until Next is enabled
    sleep(Duration::from_secs(8)).await;
    snapshot(page, "after next").await;
    Ok(())
}

async fn run(page: &Page) -> Result<()> {
    // Warm up the signed-in session first.
    page.goto(HOME_URL).await?;
    sleep(Duration::from_secs(8)).await;
    snapshot(page, "warmup").await;

    for attempt in 1..=3 {
        page.goto(LINK).await?;
        sleep(Duration::from_secs(6)).await;
        dismiss_popups(page).await;
        snapshot(page, "after goto").await;

        if is_guest_page(page).await {
            if let Err(e) = sign_in(page).await {
                eprintln!("sign-in failed: {e:#}");
            }
            dismiss_popups(page).await;
        }

        snapshot(page, "before join").await;

        // Join counts only if a Leave button shows up afterwards.
        let joined = click_button(page, "Join now", 25).await.is_ok()
            && find_button(page, "Leave|Hang up", 30, false).await.is_ok();

        if joined {
            eprintln!("joined, staying {STAY_MINUTES} minutes");
            sleep(Duration::from_secs(STAY_MINUTES * 60)).await;
            return Ok(());
        }

        eprintln!("attempt {attempt} failed, url = {:?}", page.url().await?);
    }
    bail!("Could not join after 3 attempts")
}

#[tokio::main]
async fn main() -> Result<()> {
    let login_only = std::env::args().any(|a| a == "--login");
    let profile: PathBuf = dirs_home().join(".config/teams-autojoin/profile-brave");

    let config = BrowserConfig::builder()
        .chrome_executable(BRAVE)
        .user_data_dir(profile)
        .with_head()
        .window_size(1280, 800)
        .viewport(None)
        .arg("--use-fake-ui-for-media-stream")
        .build()
        .map_err(|e| anyhow!(e))?;

    let (mut browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move {
        while let Some(_event) = handler.next().await {
            // ignore non-fatal errors, keep loop alive
        }
    });

    if login_only {
        let _page = browser.new_page(HOME_URL).await?;
        println!("Sign in fully (including MFA), then press Enter here...");
        let mut s = String::new();
        std::io::stdin().read_line(&mut s)?;
        browser.close().await?;
        return Ok(());
    }

    let page = browser.new_page("about:blank").await?;
    if let Err(e) = run(&page).await {
        eprintln!("ERROR: {e:#}");
        alarm(&format!("Auto-join failed: {e}. Join manually NOW!"));
        sleep(Duration::from_secs(60 * 60)).await;
    }
    browser.close().await?;

    Ok(())
}