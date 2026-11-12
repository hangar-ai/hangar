//! A pane's shell started, and its agent's command typed into it once the
//! shell has settled: what the desktop's pane controller does when a pane
//! mounts (`panes/controller.rs`), with no terminal to measure.
//!
//! A Claude Code pane gets the hangar-bridge mod (the settings'
//! `claude_bridge`), the account its pane, workspace or settings chose, its
//! session resumed when its transcript is there, and, for a workspace that
//! chose its MCP servers or a pane with a role, the `--mcp-config` file
//! core writes (`mcp_scope_write`), named by the variable the shell is
//! handed. An orchestrator's or a worker's pane starts with the
//! orchestration's permission mode and Hangar's tools let through.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::terminals::{Spawn, TermEvent, TermSink};
use super::{EngineHost, locked};
use crate::live::PaneStatus;
use crate::panes::agents::{LaunchOptions, launch_command};
use crate::panes::wall_clock_ms;
use crate::state::roles::{ORCHESTRATION_ALLOWED_TOOLS, orchestration_of};
use crate::state::types::{AgentId, McpMode, Pane, Workspace};

/// How long a shell may stay quiet after it first printed before it is
/// taken as ready for the agent's command.
const SETTLED: Duration = Duration::from_millis(500);
/// The longest a launch waits for its shell.
const LAUNCH_WAIT: Duration = Duration::from_secs(8);
const POLL: Duration = Duration::from_millis(50);

/// The variable a pane's shell holds its `--mcp-config` file in, as a POSIX
/// shell reads it: the servers' folder holds the person's name, which the
/// typed command never shows.
const MCP_ARG: &str = "\"$HANGAR_MCP_CONFIG\"";

/// The folder a pane's shell opens in: its own, else its workspace's.
pub fn folder_of(ws: &Workspace, pane: &Pane) -> String {
    pane.cwd
        .as_deref()
        .filter(|cwd| !cwd.is_empty())
        .unwrap_or(&ws.cwd)
        .to_string()
}

/// What the pane's launch was made of.
struct Prepared {
    spawn: Spawn,
    /// The `--mcp-config` argument and whether it replaces Claude Code's own
    /// servers.
    mcp: Option<bool>,
    resume: Option<String>,
}

/// Off tokio's async threads: the accounts, the MCP scope and the
/// transcripts are files.
fn prepare(host: &EngineHost, ws: &Workspace, pane: &Pane) -> Prepared {
    let settings = host.snapshot().state.settings.clone();
    let cwd = folder_of(ws, pane);
    let claude = pane.agent == AgentId::Claude;
    let login = claude && pane.login == Some(true);
    let account = claude.then(|| {
        crate::accounts::launch_account(
            settings.claude_account.as_deref(),
            ws.claude_account.as_deref(),
            pane.claude_account.as_deref(),
        )
        .id
    });
    let custom = ws
        .mcp
        .as_ref()
        .filter(|mcp| mcp.mode == McpMode::Custom)
        .map(|mcp| mcp.enabled.clone());
    let scope = if !claude || login {
        None
    } else if let Some(enabled) = custom {
        write_scope(host, ws, &cwd, enabled, account.clone()).map(|path| (path, true))
    } else if pane.role.is_some() {
        write_scope(host, ws, &cwd, Vec::new(), account.clone()).map(|path| (path, false))
    } else {
        None
    };
    let build_env = if pane.port_base.is_some() {
        hangar_core::build_cache::env_for(
            std::path::Path::new(&ws.cwd),
            &ws.id,
            orchestration_of(ws).shared_build_cache,
        )
    } else {
        Vec::new()
    };
    // A transcript not found is not resumed.
    let resume = pane
        .session_id
        .clone()
        .filter(|id| !id.is_empty())
        .filter(|id| hangar_core::sessions::session_exists(id.clone()) != Some(false));
    Prepared {
        spawn: Spawn {
            id: pane.id.clone(),
            cwd,
            name: pane.name.clone(),
            agent: pane.agent.as_str().to_string(),
            bridge: settings.claude_bridge,
            port_base: pane.port_base.and_then(|port| u16::try_from(port).ok()),
            build_env,
            mcp_config: scope.as_ref().map(|(path, _)| path.clone()),
            claude_account: account,
        },
        mcp: scope.map(|(_, strict)| strict),
        resume,
    }
}

fn write_scope(
    host: &EngineHost,
    ws: &Workspace,
    cwd: &str,
    enabled: Vec<String>,
    account: Option<String>,
) -> Option<String> {
    match hangar_core::mcp_scope::mcp_scope_write(
        host.core(),
        ws.id.clone(),
        ws.cwd.clone(),
        cwd.to_string(),
        enabled,
        None,
        account,
    ) {
        Ok(file) => Some(file.path),
        Err(error) => {
            eprintln!(
                "workspace {}: the MCP servers of a pane could not be written ({error}); it starts with its own",
                ws.id
            );
            None
        }
    }
}

/// Starts `pane` of `ws`: its machine, its shell, then its agent. `launch`
/// is what holds the pane for this start: a pane that left the state (a
/// worker's start undone, a workspace removed or put to sleep) while its
/// shell was starting is no longer held by it, and the shell is taken
/// down again, which nothing else would do.
pub(super) async fn start(host: EngineHost, ws: Workspace, pane: Pane, launch: u64) {
    if !host.held_by(&pane.id, launch) {
        return;
    }
    host.mounted(&pane, &ws);
    let prepared = {
        let (host, ws, pane) = (host.clone(), ws.clone(), pane.clone());
        tokio::task::spawn_blocking(move || prepare(&host, &ws, &pane)).await
    };
    let Ok(prepared) = prepared else {
        return failed(&host, &pane, launch, "the launch stopped");
    };
    let terminals = host.terminals().clone();
    // What the pane prints is noted where its reader thread stands, with
    // no lock; only its end goes through the host.
    let sink: TermSink = {
        let host = host.clone();
        let printed = host.printed_of(&pane.id);
        Arc::new(move |event| match event {
            TermEvent::Output(_) => printed.note(wall_clock_ms()),
            exit @ TermEvent::Exit(_) => host.on_term(exit),
        })
    };
    let spawn = prepared.spawn.clone();
    let started = tokio::task::spawn_blocking(move || terminals.spawn(spawn, sink)).await;
    match started {
        Ok(Ok(())) if !host.held_by(&pane.id, launch) => {
            // Removed meanwhile: the kill that came with the removal found
            // no shell yet.
            let (terminals, id) = (host.terminals().clone(), pane.id.clone());
            let _ = tokio::task::spawn_blocking(move || terminals.kill(&id)).await;
            // Unless another launch holds the pane since (a workspace woken
            // again): its machine is that launch's.
            if !locked(&host.0.shells).contains_key(&pane.id) {
                host.unmounted(&pane.id);
            }
            return;
        }
        Ok(Ok(())) => host.set_status(&pane.id, PaneStatus::Running),
        Ok(Err(error)) => return failed(&host, &pane, launch, &error),
        Err(error) => return failed(&host, &pane, launch, &error.to_string()),
    }
    let snapshot = host.snapshot();
    let Some(ws) = snapshot.state.workspaces.iter().find(|w| w.id == ws.id) else {
        return;
    };
    let Some(pane) = ws.panes.iter().find(|p| p.id == pane.id) else {
        return;
    };
    let Some(line) = command_line(ws, pane, &snapshot.state.settings, &prepared) else {
        return;
    };
    settle(&host, &pane.id).await;
    let terminals = host.terminals().clone();
    let id = pane.id.clone();
    let typed =
        tokio::task::spawn_blocking(move || terminals.write(&id, &format!("{line}\r"))).await;
    match typed {
        Ok(Ok(())) => host.launched(&pane.id, prepared.resume.as_deref()),
        Ok(Err(error)) => eprintln!("pane {}: its agent could not be started: {error}", pane.id),
        Err(error) => eprintln!("pane {}: its agent could not be started: {error}", pane.id),
    }
}

/// The command typed into the shell: `None` for a plain shell.
fn command_line(
    ws: &Workspace,
    pane: &Pane,
    settings: &crate::state::types::Settings,
    prepared: &Prepared,
) -> Option<String> {
    let (permission_mode, tools): (Option<String>, Vec<String>) = if pane.role.is_some() {
        (
            orchestration_of(ws).permission_mode,
            ORCHESTRATION_ALLOWED_TOOLS
                .iter()
                .map(|tool| (*tool).to_owned())
                .collect(),
        )
    } else {
        (None, Vec::new())
    };
    let name = crate::state::pane_names::session_name(&pane.name, &ws.name);
    let options = LaunchOptions {
        extra_roots: &ws.extra_roots,
        command: settings.agent_commands.claude.as_deref(),
        name: Some(&name),
        mcp_config: prepared.mcp.map(|_| MCP_ARG),
        mcp_strict: prepared.mcp,
        permission_mode: permission_mode.as_deref(),
        allowed_tools: &tools,
        model: pane.model.as_deref(),
    };
    launch_command(pane.agent, prepared.resume.as_deref(), &options)
}

/// Waits for the shell to print its prompt and fall quiet, or for
/// [`LAUNCH_WAIT`]: a command typed before the shell reads its input can be
/// lost, or echoed twice.
async fn settle(host: &EngineHost, pane: &str) {
    let began = Instant::now();
    loop {
        if began.elapsed() >= LAUNCH_WAIT {
            return;
        }
        if let Some(at) = host.last_printed(pane)
            && wall_clock_ms().saturating_sub(at) >= SETTLED.as_millis() as u64
        {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
}

/// The shell could not start: the pane says so and stays down. Not when
/// the pane is no longer this launch's (it left the state meanwhile).
fn failed(host: &EngineHost, pane: &Pane, launch: u64, error: &str) {
    if !host.held_by(&pane.id, launch) {
        return;
    }
    eprintln!("pane {} ({}) could not start: {error}", pane.id, pane.name);
    host.set_status(&pane.id, PaneStatus::Exited);
}
