use std::process::Command;

pub fn get_pane_width() -> Option<usize> {
    if std::env::var("TMUX").is_err() {
        return None;
    }

    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{pane_width}"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<usize>()
        .ok()
}
