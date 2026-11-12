//! `serve` and `serve --detach`: one daemon per account, found again through
//! `node.json` and kept unique by the lock it holds (state.rs).
#![cfg(target_os = "linux")]

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::serve::ListenerExt;
use hangar_core::Host;

use crate::state::{self, NodeFile};

/// How long a `serve` waits on a daemon another one is starting, and
/// `--detach` on the one it started: the login shell's PATH is read first,
/// which a slow profile can stretch.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// A look at `node.json` costs a failed open until the daemon writes it:
/// often, so that `--detach` returns as soon as it answers (the desktop's
/// connection waits on it).
const POLL: Duration = Duration::from_millis(5);

/// How often a `serve --supervised` beside a daemon started outside its
/// service looks whether that one is still there.
const SUPERVISED_POLL: Duration = Duration::from_secs(2);

/// The node in the foreground, until SIGTERM, SIGINT or SIGHUP. A daemon of
/// the account's already running and answering is success: there is one.
///
/// `supervised` (the systemd unit's `serve --supervised`): a daemon that
/// already runs was started outside the service (by the desktop over SSH,
/// which does so after it upgrades the node, or by hand). Leaving then
/// would end the unit, and nothing would start the node again when that
/// daemon stops: this one waits beside it instead, and becomes the node
/// when the lock is free.
pub fn serve(supervised: bool) -> Result<(), String> {
    let dir = state::dir()?;
    state::ensure_dir(&dir)?;
    if !supervised {
        if let Some(node) = state::running(&dir) {
            eprintln!("hangar-node is already running (pid {})", node.pid);
            return Ok(());
        }
    }
    let lock = match state::try_lock(&dir)? {
        Some(lock) => lock,
        None if supervised => {
            eprintln!("hangar-node already runs outside this service: taking over when it stops");
            loop {
                std::thread::sleep(SUPERVISED_POLL);
                if let Some(lock) = state::try_lock(&dir)? {
                    break lock;
                }
            }
        }
        None => {
            // Another `serve` holds the lock and is starting: it is the node.
            let node = wait_for_running(&dir, || None)?;
            eprintln!("hangar-node is already running (pid {})", node.pid);
            return Ok(());
        }
    };
    // Whatever `node.json` is left names no daemon: that one held the lock.
    let _ = std::fs::remove_file(state::node_file(&dir));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("no runtime: {err}"))?;
    let host = Host::new();
    let headless = hangar_core::headless::start(
        host.clone(),
        runtime.handle(),
        Some(state::endpoint_file(&dir)),
    )?;
    let outcome = runtime.block_on(run(&dir, host, &headless));
    state::clear(&dir);
    headless.shutdown();
    // What still waits on the network (a call to Discord that is slow, or
    // told to wait) is not waited for: dropped as it is, the runtime would
    // hold the daemon's end, a `systemctl stop` with it, for as long. The
    // lock is kept until then: no other daemon starts beside this one.
    runtime.shutdown_timeout(Duration::from_secs(2));
    drop(lock);
    outcome
}

async fn run(
    dir: &Path,
    host: Host,
    headless: &hangar_core::headless::Headless,
) -> Result<(), String> {
    let token = hangar_core::endpoint::generate_token();
    let panes = Arc::new(crate::panes::Panes::new(headless.pty().clone()));
    // The workspaces the node runs itself: an unreadable state file stops
    // the node rather than letting the first save write over it.
    let own = hangar_engine::host::EngineHost::load(hangar_engine::host::Config {
        core: host.clone(),
        terminals: Arc::new(crate::own::NodeTerminals::new(
            panes.clone(),
            headless.pty().clone(),
        )),
        state_file: state::engine_file(dir),
    })?;
    let node = Arc::new(crate::api::Node::with_panes(
        token.clone(),
        host,
        panes,
        crate::proxy::Core {
            port: headless.port(),
            token: headless.token().to_string(),
        },
        Some(own.clone()),
    ));
    crate::control::watch(node.host(), node.drivers.clone(), Some(own.clone()));
    own.start();
    node.discord.restart(&own, &node.drivers, dir);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|err| format!("the node API could not listen: {err}"))?;
    let port = listener.local_addr().map_err(|err| err.to_string())?.port();
    state::write(
        dir,
        &NodeFile {
            version: crate::VERSION.to_string(),
            port,
            token,
            pid: std::process::id(),
        },
    )?;
    eprintln!(
        "hangar-node {} listening on 127.0.0.1:{port} (pid {})",
        crate::VERSION,
        std::process::id()
    );
    let app = crate::api::router(node.clone());
    // A key's echo is a few bytes: sent at once, not held back by Nagle's
    // algorithm until the last segment is acknowledged.
    let listener = listener.tap_io(|tcp| {
        let _ = tcp.set_nodelay(true);
    });
    axum::serve(listener, app)
        .with_graceful_shutdown(stopped())
        .await
        .map_err(|err| format!("the node API stopped: {err}"))?;
    eprintln!("hangar-node stopping");
    own.shutdown().await;
    node.panes.kill_all();
    Ok(())
}

/// SIGTERM (systemd, `kill`), SIGINT, or SIGHUP (the terminal of a `serve`
/// run by hand closing).
async fn stopped() {
    use tokio::signal::unix::{signal, SignalKind};
    async fn on(kind: SignalKind) {
        match signal(kind) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    }
    tokio::select! {
        _ = on(SignalKind::terminate()) => {}
        _ = on(SignalKind::interrupt()) => {}
        _ = on(SignalKind::hangup()) => {}
    }
}

/// `serve` again, in a session of its own (setsid) with its output appended
/// to `node.log`, and back once the account's daemon answers — this one, or
/// one that was already there.
pub fn detach() -> Result<(), String> {
    let dir = state::dir()?;
    state::ensure_dir(&dir)?;
    if state::running(&dir).is_some() {
        return Ok(());
    }
    let log_path = state::log_file(&dir);
    let log = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&log_path)
            .map_err(|err| format!("{}: {err}", log_path.display()))?
    };
    let err_log = log.try_clone().map_err(|err| err.to_string())?;
    let exe = std::env::current_exe().map_err(|err| format!("no executable: {err}"))?;
    let mut command = Command::new(exe);
    command
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(err_log);
    // SAFETY: setsid is async-signal-safe, as pre_exec requires.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("could not start the daemon: {err}"))?;
    wait_for_running(&dir, || match child.try_wait() {
        // Exit 0: it found a daemon already there, which the wait finds too.
        Ok(Some(status)) if !status.success() => Some(format!(
            "the daemon exited ({status}); see {}",
            log_path.display()
        )),
        _ => None,
    })
    .map(|_| ())
}

/// Polls until the account's daemon answers, `failed` says why it never
/// will, or the time runs out.
fn wait_for_running(
    dir: &Path,
    mut failed: impl FnMut() -> Option<String>,
) -> Result<NodeFile, String> {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if let Some(node) = state::running(dir) {
            return Ok(node);
        }
        if let Some(reason) = failed() {
            return Err(reason);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "no daemon answered within {}s; see {}",
                START_TIMEOUT.as_secs(),
                state::log_file(dir).display()
            ));
        }
        std::thread::sleep(POLL);
    }
}
