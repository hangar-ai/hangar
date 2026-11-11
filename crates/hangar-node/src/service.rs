//! `hangar-node service install`: the daemon as a systemd user service, for
//! a node that runs on its own (docs/native/headless.md, step 5): started
//! with the account, started again if it stops, kept after the person logs
//! out once lingering is on.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The unit, running `exe serve` in the foreground.
pub fn unit(exe: &Path) -> String {
    format!(
        "[Unit]\n\
Description=Hangar node: its workspaces and the Discord connector\n\
After=network-online.target\n\
Wants=network-online.target\n\
\n\
[Service]\n\
ExecStart=\"{}\" serve\n\
Restart=on-failure\n\
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
    if systemctl(&["daemon-reload"]) && systemctl(&["enable", "--now", "hangar-node.service"]) {
        println!("hangar-node runs as a service now (systemctl --user status hangar-node).");
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
        assert!(unit
            .contains("ExecStart=\"/home/me/.local/share/hangar-node/bin/hangar-node\" serve\n"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("WantedBy=default.target"));
    }
}
