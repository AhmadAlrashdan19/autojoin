use chromiumoxide::Page;

pub async fn snapshot(page: &Page, tag: &str) {
    let js = r#"JSON.stringify({
        url: location.href,
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