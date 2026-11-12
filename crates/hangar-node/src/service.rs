//! `hangar-node service install`: the daemon as a systemd user service, for
//! a node that runs on its own (docs/native/headless.md, step 5): started
//! with the account, started again if it stops, kept after the person logs
//! out once lingering is on.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The unit, running `exe serve --supervised` in the foreground: started
/// again however the daemon ends (the desktop stops it with a plain `kill`
/// when it upgrades the node, which is a clean end), and waiting beside a
/// daemon started outside the service until that one stops (daemon.rs).
pub fn unit(exe: &Path) -> String {
    format!(
        "[Unit]\n\
Description=Hangar node: its workspaces and the Discord connector\n\
\n\
[Service]\n\
ExecStart=\"{}\" serve --supervised\n\
Restart=always\n\
RestartSec=5\n\
\n\
[Install]\n\
WantedBy=default.target\n",
        exe.display()
    )
}

fn unit_file() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("no home directory")?;
    Ok(home
        .join(".config")
        .join("systemd")
        .join("user")
        .join("hangar-node.service"))
}

pub fn install() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| format!("no executable: {err}"))?;
    let file = unit_file()?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    std::fs::write(&file, unit(&exe)).map_err(|err| format!("{}: {err}", file.display()))?;
    println!("Wrote {}", file.display());
    let systemctl = |args: &[&str]| {
        Command::new("systemctl")
            .arg("--user")
            .args(args)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    };
    // A daemon that already runs (the desktop started it) keeps running,
    // its agents with it: the service waits beside it.
    let already = crate::state::dir()
        .ok()
        .and_then(|dir| crate::state::running(&dir));
    if systemctl(&["daemon-reload"]) && systemctl(&["enable", "--now", "hangar-node.service"]) {
        match already {
            Some(node) => println!(
                "hangar-node is a service now. The node that already runs (pid {}) stays as it is, its agents with it: the service takes over when it stops, and at each login.",
                node.pid
            ),
            None => println!(
                "hangar-node runs as a service now (systemctl --user status hangar-node)."
            ),
        }
        println!("To keep it running once you log out: loginctl enable-linger $USER");
    } else {
        println!("systemd did not take it here: start it with `hangar-node serve --detach`,");
        println!("or `systemctl --user enable --now hangar-node.service` where systemd runs.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unit_runs_serve_in_the_foreground_and_restarts_it() {
        let unit = unit(Path::new(
            "/home/me/.local/share/hangar-node/bin/hangar-node",
        ));
        assert!(unit.contains(
            "ExecStart=\"/home/me/.local/share/hangar-node/bin/hangar-node\" serve --supervised\n"
        ));
        // However it ends: the desktop's `kill` before an upgrade is a
        // clean end, which `on-failure` would not start again.
        assert!(unit.contains("Restart=always"));
        assert!(unit.contains("WantedBy=default.target"));
        // A user unit has no network-online.target to wait for.
        assert!(!unit.contains("network-online"));
    }
}
