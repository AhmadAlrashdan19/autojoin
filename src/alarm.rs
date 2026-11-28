use tokio::{process::Command, task::JoinHandle};

pub struct AlarmHandle(JoinHandle<()>);

impl AlarmHandle {
    pub fn stop(&self) {
        self.0.abort();
    }
}

async fn notify_run(cmd: &str, args: &[&str]) {
    // kill_on_drop: aborting the task also kills a running paplay.
    match Command::new(cmd).args(args).kill_on_drop(true).status().await {
        Ok(s) if s.success() => {},
        Ok(s) => eprintln!("Alarm: {cmd} exited with {s}"),
        Err(e) => eprintln!("Alarm: Cannot run {cmd}: {e}"),
    }
}

pub async fn alarm(msg: &str) {
    notify_run("notify-send", &["-u", "critical", "Teams", msg]).await;
    // Repeat the voice to be really heard
    for _ in 0..3 {
        notify_run("paplay", &["/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga"]).await;
    }
}

/// Non-blocking alarm that can be silenced with `.stop()`.
pub fn alarm_start(msg: &str) -> AlarmHandle {
    let msg = msg.to_string();
    AlarmHandle(tokio::spawn(async move { alarm(&msg).await }))
}
