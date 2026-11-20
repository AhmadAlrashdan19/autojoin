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
        alarm(&format!("teams-autojoin failed to start: {e}"));
        std::process::exit(1);
    }
}
