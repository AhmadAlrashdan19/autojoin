mod alarm;
mod bootstrap;
mod browser;
mod config;
mod snapshot;
mod teams;

use crate::alarm::alarm;
use crate::bootstrap::bootstrap;

#[tokio::main]
async fn main() {
    if let Err(e) = bootstrap().await {
        eprintln!("FATAL: {e:#?}");
        alarm(&format!("autojoin failed to start: {e}")).await;
        std::process::exit(1);
    }
}
