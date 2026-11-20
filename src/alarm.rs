use tokio::process::Command;

async fn notify_run(cmd: &str, args: &[&str]) {
    match Command::new(cmd).args(args).status().await {
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
