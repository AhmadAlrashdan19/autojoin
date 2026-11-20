use std::process::Command;

pub fn alarm(msg: &str) {
    let _ = Command::new("notify-send")
        .args(["-u", "critical", "Teams", msg])
        .status();
    let _ = Command::new("paplay")
        .arg("/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga")
        .status();
}
