use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

const TELEMETRY_MARKER_FILE: &str = ".telemetry_installed";
const TELEMETRY_PING_URL: &str = "https://hits.sh/github.com/nhatcoi/procman-unique-users.svg";

fn get_telemetry_marker_path() -> Option<PathBuf> {
    let state_dir = dirs::state_dir()
        .or_else(dirs::data_local_dir)?
        .join("procman");
    Some(state_dir.join(TELEMETRY_MARKER_FILE))
}

/// Checks if this is the first time procman is run on this machine.
/// If so, marks it as installed and fires an anonymous background telemetry ping.
/// This runs in a detached thread and will never block CLI execution.
pub fn check_first_run() {
    // Support standard opt-out environment variables
    if std::env::var("DO_NOT_TRACK").unwrap_or_default() == "1"
        || std::env::var("PROCMAN_NO_TELEMETRY").unwrap_or_default() == "1"
    {
        return;
    }

    let Some(marker_path) = get_telemetry_marker_path() else {
        return;
    };

    if marker_path.exists() {
        return;
    }

    // Ensure parent directory exists and create marker immediately
    if let Some(parent) = marker_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&marker_path, b"installed\n");

    // Fire-and-forget anonymous telemetry ping in a detached background thread
    thread::spawn(move || {
        let _ = Command::new("curl")
            .args(["-s", "-m", "2", TELEMETRY_PING_URL])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_marker_path_resolves() {
        let path = get_telemetry_marker_path();
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.ends_with(TELEMETRY_MARKER_FILE));
    }
}
