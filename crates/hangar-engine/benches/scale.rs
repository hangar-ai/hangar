//! What grows with what a server holds: `cargo bench -p hangar-engine
//! --bench scale`.
//!
//! - `memory`: what a pane's screen weighs once its history is full, for
//!   the history kept and for shorter ones (resident memory, per pane);
//! - `state`: a model write, a save and a load as the state grows (8, 50
//!   and 200 workspaces of 6 panes);
//! - `transcript`: a page of a conversation read from a transcript of 1, 10
//!   and 50 MB, the first time and again (core keeps what it read);
//! - `board`: a board of 100, 500 and 2,000 tasks read, as the
//!   orchestration reads each workspace's every 10 s;
//! - `idle`: the processor a host of 48 panes uses with nothing happening.
//!
//! Options, after `--`: `--only <name>`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::json;

use hangar_engine::control::port::ModelCmd;
use hangar_engine::host::screen::Screen;
use hangar_engine::host::{Config, EngineHost, Spawn, TermSink, Terminals};
use hangar_engine::state::PanePatch;
use hangar_engine::state::types::{AgentId, AppState, Pane, PaneRole, Settings, Workspace};

#[derive(Default)]
struct Shells {
    started: Mutex<HashMap<String, TermSink>>,
}

impl Terminals for Shells {
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String> {
        self.started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(spawn.id, sink);
        Ok(())
    }
    fn write(&self, _pane: &str, _data: &str) -> Result<(), String> {
        Ok(())
    }
    fn alive(&self, _pane: &str) -> bool {
        true
    }
    fn kill(&self, _pane: &str) -> Result<(), String> {
        Ok(())
    }
    fn lines(&self, _pane: &str, _n: usize) -> Option<Vec<String>> {
        Some(Vec::new())
    }
    fn screen(&self, _pane: &str) -> Option<Vec<String>> {
        Some(Vec::new())
    }
    fn kill_all(&self) {}
}

fn option(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|at| args.get(at + 1).cloned())
}

fn wanted(name: &str) -> bool {
    option("--only").is_none_or(|only| only == name)
}

/// Resident memory of this process, in bytes.
fn resident() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    statm
        .split_whitespace()
        .nth(1)
        .and_then(|pages| pages.parse::<u64>().ok())
        .unwrap_or(0)
        * 4096
}

/// The processor time this process used so far, in ms.
fn cpu_ms() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let rest = stat.rsplit_once(')').map(|(_, rest)| rest).unwrap_or("");
    let fields: Vec<&str> = rest.split_whitespace().collect();
    [11, 12]
        .iter()
        .filter_map(|i| fields.get(*i)?.parse::<u64>().ok())
        .sum::<u64>()
        * 10
}

/// Lines an agent prints, each of its own text, about 90 columns wide.
fn lines(count: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for n in 0..count {
        out.extend_from_slice(
            format!(
                "\x1b[38;5;244m{n:>6}\x1b[0m  let value_{n} = compute(&input, {n})?; // step {n} of the build, checked\r\n"
            )
            .as_bytes(),
        );
    }
    out
}

fn bench_memory() {
    let panes = 16usize;
    let output = lines(12_000);
    // Kept until the end: memory freed by one size would hide the next.
    let mut kept = Vec::new();
    for history in [200usize, 1_000, 2_000, 5_000] {
        let before = resident();
        let mut screens: Vec<Screen> = (0..panes)
            .map(|_| Screen::with_history(120, 40, history))
            .collect();
        for screen in &mut screens {
            for chunk in output.chunks(8 * 1024) {
                screen.feed(chunk);
            }
        }
        let grown = resident().saturating_sub(before);
        // What 1,000 lines asked of it read back (`pane_read`'s most).
        let read = screens[0].lines(1_000).len();
        println!(
            "memory   a pane's screen, {history:>5} rows of history, full   {:>6.2} MiB per pane   ({read} lines read back)",
            grown as f64 / panes as f64 / (1024.0 * 1024.0)
        );
        kept.push(screens);
    }
}

fn state_of(workspaces: usize, per: usize, cwd: &str) -> (AppState, Vec<String>) {
    let mut panes = Vec::new();
    let state = AppState {
        workspaces: (0..workspaces)
            .map(|w| Workspace {
                id: format!("w{w}"),
                name: format!("Workspace {w}"),
                cwd: cwd.to_string(),
                panes: (0..per)
                    .map(|p| {
                        let id = format!("w{w}-p{p}");
                        panes.push(id.clone());
                        Pane {
                            id,
                            name: format!("Agent {p}"),
                            agent: AgentId::Claude,
                            role: Some(if p == 0 {
                                PaneRole::Orchestrator
                            } else {
                                PaneRole::Worker
                            }),
                            session_id: Some(format!("00000000-0000-4000-8000-{w:06}{p:06}")),
                            ..Pane::default()
                        }
                    })
                    .collect(),
                ..Workspace::default()
            })
            .collect(),
        settings: Settings {
            claude_bridge: true,
            ..Settings::default()
        },
        ..AppState::default()
    };
    (state, panes)
}

struct Rig {
    host: EngineHost,
    panes: Vec<String>,
    file: std::path::PathBuf,
    _root: tempfile::TempDir,
}

fn rig(runtime: &tokio::runtime::Runtime, workspaces: usize, per: usize, start: bool) -> Rig {
    let root = tempfile::tempdir().unwrap();
    let (state, panes) = state_of(workspaces, per, &root.path().to_string_lossy());
    let file = root.path().join("node").join("state.json");
    hangar_engine::host::write_state_file(&file, &serde_json::to_value(&state).unwrap()).unwrap();
    let shells = Arc::new(Shells::default());
    let host = EngineHost::load(Config {
        core: hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(root.path())),
        terminals: shells.clone(),
        state_file: file.clone(),
    })
    .unwrap();
    if start {
        let _guard = runtime.enter();
        host.start();
        let deadline = Instant::now() + Duration::from_secs(60);
        while shells
            .started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
            < panes.len()
        {
            assert!(Instant::now() < deadline, "the panes never started");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Rig {
        host,
        panes,
        file,
        _root: root,
    }
}

fn bench_state(runtime: &tokio::runtime::Runtime) {
    for workspaces in [8usize, 50, 200] {
        let rig = rig(runtime, workspaces, 6, false);
        let writes = 300usize;
        let began = Instant::now();
        for n in 0..writes {
            let pane = &rig.panes[(n * 7) % rig.panes.len()];
            let ws = pane.split('-').next().unwrap_or_default().to_string();
            let model = format!("model-{n}");
            rig.host
                .apply(ModelCmd::UpdatePane {
                    ws,
                    pane: pane.clone(),
                    patch: PanePatch::new(move |pane: &mut Pane| pane.model = Some(model.clone())),
                })
                .unwrap();
        }
        let write = began.elapsed().as_secs_f64() * 1e6 / writes as f64;
        let saves = 20usize;
        let began = Instant::now();
        for n in 0..saves {
            let pane = &rig.panes[n % rig.panes.len()];
            let ws = pane.split('-').next().unwrap_or_default().to_string();
            let name = format!("Agent renamed {n}");
            rig.host
                .apply(ModelCmd::UpdatePane {
                    ws,
                    pane: pane.clone(),
                    patch: PanePatch::new(move |pane: &mut Pane| pane.name = name.clone()),
                })
                .unwrap();
            runtime.block_on(rig.host.save()).unwrap();
        }
        let save = began.elapsed().as_secs_f64() * 1e3 / saves as f64;
        let size = std::fs::metadata(&rig.file).map(|m| m.len()).unwrap_or(0);
        let loads = 20usize;
        let began = Instant::now();
        for _ in 0..loads {
            let loaded = EngineHost::load(Config {
                core: rig.host.core().clone(),
                terminals: Arc::new(Shells::default()),
                state_file: rig.file.clone(),
            })
            .unwrap();
            assert_eq!(loaded.snapshot().state.workspaces.len(), workspaces);
        }
        let load = began.elapsed().as_secs_f64() * 1e3 / loads as f64;
        println!(
            "state    {workspaces:>3} workspaces of 6 panes ({:>4} KiB)   a write {write:>7.1} us   a save {save:>6.2} ms   a load {load:>6.2} ms",
            size / 1024
        );
    }
}

/// A transcript of about `megabytes`, as Claude Code writes them, under the
/// home of this run; its folder name is the cwd's with dashes.
fn write_transcript(home: &std::path::Path, cwd: &str, session: &str, megabytes: usize) -> u64 {
    use std::io::Write;
    let folder: String = cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let dir = home.join(".claude").join("projects").join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{session}.jsonl"));
    let mut out = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    let base = |kind: &str, n: usize| {
        json!({
            "parentUuid": null, "isSidechain": false, "type": kind,
            "uuid": format!("u{n}"), "timestamp": "2026-11-06T05:00:00.000Z",
            "cwd": cwd, "sessionId": session, "version": "2.9.0",
        })
    };
    let mut written = 0u64;
    let mut n = 0usize;
    let diff =
        "+    let total = items.iter().map(|item| item.price * item.quantity).sum::<u64>();\n"
            .repeat(40);
    while written < (megabytes as u64) * 1024 * 1024 {
        n += 1;
        let mut user = base("user", n * 4);
        user["message"] = json!({ "role": "user", "content": format!("Step {n}: add the cart's total and its test.") });
        let mut call = base("assistant", n * 4 + 1);
        call["message"] = json!({ "role": "assistant", "content": [
            { "type": "text", "text": format!("I add the total in cart.rs, then its test (step {n}).") },
            { "type": "tool_use", "id": format!("tu{n}"), "name": "Edit",
              "input": { "file_path": "src/cart.rs", "old_string": "// total", "new_string": diff } },
        ]});
        let mut result = base("user", n * 4 + 2);
        result["message"] = json!({ "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": format!("tu{n}"), "content": "The file src/cart.rs has been updated." },
        ]});
        let mut answer = base("assistant", n * 4 + 3);
        answer["message"] = json!({ "role": "assistant", "content": [
            { "type": "text", "text": format!("Done: the cart's total is computed and tested (step {n}).") },
        ]});
        for line in [user, call, result, answer] {
            let text = format!("{line}\n");
            written += text.len() as u64;
            out.write_all(text.as_bytes()).unwrap();
        }
    }
    out.flush().unwrap();
    written
}

fn bench_transcript(runtime: &tokio::runtime::Runtime) {
    let home = tempfile::tempdir().unwrap();
    // Before anything of core reads where Claude Code keeps its files.
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    let cwd = "/srv/shop";
    for megabytes in [1usize, 10, 50] {
        let session = format!("00000000-0000-4000-8000-{megabytes:012}");
        let size = write_transcript(home.path(), cwd, &session, megabytes);
        let page = || {
            runtime
                .block_on(hangar_core::chat::chat_page(
                    session.clone(),
                    cwd.to_string(),
                    None,
                    Some(40),
                ))
                .unwrap()
        };
        let began = Instant::now();
        let first = page();
        let cold = began.elapsed();
        let items = first.map(|page| page.items.len()).unwrap_or(0);
        let again = 200usize;
        let began = Instant::now();
        for _ in 0..again {
            page();
        }
        let warm = began.elapsed().as_secs_f64() * 1e6 / again as f64;
        println!(
            "transcript  {:>3} MB   the newest page ({items} items) the first time {:>7.2} ms   again {warm:>7.1} us",
            size / (1024 * 1024),
            cold.as_secs_f64() * 1e3
        );
    }
}

fn bench_board() {
    for tasks in [100usize, 500, 2_000] {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().to_string_lossy().into_owned();
        for n in 0..tasks {
            hangar_core::board_create(
                cwd.clone(),
                serde_json::from_value(json!({
                    "title": format!("Task {n}: the search index rebuilt from the journal"),
                    "description": "Rebuild the index from the journal, swap it in, and keep readers on the old one meanwhile.",
                }))
                .unwrap(),
            )
            .unwrap();
        }
        let reads = 50usize;
        let began = Instant::now();
        let mut read = 0;
        for _ in 0..reads {
            read = hangar_core::board_load(cwd.clone()).tasks.len();
        }
        let size = std::fs::metadata(root.path().join(".hangar").join("board.json"))
            .map(|m| m.len())
            .unwrap_or(0);
        println!(
            "board    {read:>5} tasks ({:>4} KiB)   read in {:>7.2} ms",
            size / 1024,
            began.elapsed().as_secs_f64() * 1e3 / reads as f64
        );
    }
}

fn bench_idle(runtime: &tokio::runtime::Runtime) {
    let rig = rig(runtime, 8, 6, true);
    // Let the launches finish what they do at the start.
    std::thread::sleep(Duration::from_secs(10));
    let seconds = 20u64;
    let before = cpu_ms();
    std::thread::sleep(Duration::from_secs(seconds));
    let spent = cpu_ms() - before;
    println!(
        "idle     a host of {} panes doing nothing   {:>5.1} ms of processor a second ({:.2} % of one core)",
        rig.panes.len(),
        spent as f64 / seconds as f64,
        spent as f64 / seconds as f64 / 10.0
    );
    runtime.block_on(rig.host.shutdown());
}

fn main() {
    if wanted("memory") {
        bench_memory();
    }
    if wanted("board") {
        bench_board();
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    if wanted("state") {
        bench_state(&runtime);
    }
    if wanted("idle") {
        bench_idle(&runtime);
    }
    // Last: it moves this process's home.
    if wanted("transcript") {
        bench_transcript(&runtime);
    }
}
