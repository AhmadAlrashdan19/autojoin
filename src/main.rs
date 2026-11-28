mod configurations;
mod bootstrap;
mod snapshot;
mod website;
mod alarm;
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
