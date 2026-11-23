use anyhow::{Result, bail};
use chromiumoxide::Page;
use std::time::Duration;
use tokio::time::sleep;

/// Wait for an ENABLED button matching the regex. Click it if `click` is true.
pub async fn has_button(page: &Page, pattern: &str, click: bool) -> bool {
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
    match page.evaluate(js.as_str()).await {
        Ok(v) => v.into_value::<bool>().unwrap_or(false),
        Err(_) => false, // page navigating, retry
    }
}

pub async fn find_button(page: &Page, pattern: &str, timeout_s: u64, click: bool) -> Result<()> {
    for _ in 0..(timeout_s * 2) {
        if has_button(page, pattern, click).await {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    bail!("Button /{pattern}/ not found or disabled");
}

pub async fn click_button(page: &Page, pattern: &str, timeout_s: u64) -> Result<()> {
    find_button(page, pattern, timeout_s, true).await
}

/// Wait for an input, click it, type text.
pub async fn type_into(page: &Page, selector: &str, text: &str, timeout_s: u64) -> Result<()> {
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
