//! What the engine's host with no window costs where it is busiest:
//! `cargo bench -p hangar-engine --bench host`.
//!
//! - `screens`: the panes' emulated screens fed at once, one thread per
//!   pane as the panes' reader threads do (8 KiB chunks of what an agent's
//!   interface prints), in MB/s for all of them together;
//! - `output`: what each chunk a pane prints costs the host beside its
//!   screen (`TermEvent::Output`), from four reader threads;
//! - `bridge`: the mod's reports (turns, tool calls, usage) taken in by a
//!   host of 8 workspaces of 6 panes, per report, and the processor time the
//!   whole process spent on them, the background's included (the panes
//!   reconciled with the state, the saves);
//! - `apply`: a model write of an engine (`ModelCmd::UpdatePane`), applied
//!   and published;
//! - `save`: the state written to its file.
//!
//! Options, after `--`: `--only <name>` runs one of them; `--workspaces N`
//! and `--panes N` size the host (8 and 6).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::json;

use hangar_engine::control::port::ModelCmd;
use hangar_engine::host::screen::Screens;
use hangar_engine::host::{Config, EngineHost, Spawn, TermEvent, TermSink, Terminals};
use hangar_engine::state::PanePatch;
use hangar_engine::state::types::{AgentId, AppState, Pane, PaneRole, Settings, Workspace};

/// Shells that only keep where their output goes.
#[derive(Default)]
struct Shells {
    sinks: Mutex<HashMap<String, TermSink>>,
}

impl Shells {
    fn sink(&self, pane: &str) -> Option<TermSink> {
        self.sinks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(pane)
            .cloned()
    }

    fn started(&self) -> usize {
        self.sinks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

impl Terminals for Shells {
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String> {
        self.sinks
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

/// What an agent's interface prints: redraws in synchronized updates, box
/// drawing, colours, cursor moves (hangar-term's `claude` sample, in short).
fn agent_output(bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes + 512);
    let mut n = 0u32;
    while out.len() < bytes {
        n += 1;
        out.extend_from_slice(b"\x1b[?2026h\x1b[2K\x1b[1G");
        out.extend_from_slice(
            format!(
                "\x1b[38;2;95;210;127m\u{256d}\u{2500}\u{2500} Read \x1b[0msrc/module_{n}.rs \x1b[2m(lines 1-{})\x1b[0m\r\n",
                n % 400
            )
            .as_bytes(),
        );
        out.extend_from_slice(
            format!(
                "\x1b[38;5;244m\u{2502}\x1b[0m   let value_{n} = compute(&input, {n}); // \u{e9}t\u{e9} \u{2192} r\u{e9}sultat\r\n"
            )
            .as_bytes(),
        );
        out.extend_from_slice("\x1b[38;5;244m\u{2570}\u{2500}\u{2500}\x1b[0m\r\n".as_bytes());
        if n.is_multiple_of(7) {
            out.extend_from_slice(b"\x1b[3A\x1b[2K\x1b[3B");
        }
        out.extend_from_slice(b"\x1b[?2026l");
    }
    out
}

/// The processor time this process used so far, user and system, in ms.
fn cpu_ms() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // After the command's closing parenthesis: state, then the fields.
    let rest = stat.rsplit_once(')').map(|(_, rest)| rest).unwrap_or("");
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let ticks: u64 = [11, 12]
        .iter()
        .filter_map(|i| fields.get(*i)?.parse::<u64>().ok())
        .sum();
    ticks * 10
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

fn bench_screens() {
    let sample = Arc::new(agent_output(2 * 1024 * 1024));
    for threads in [1usize, 4, 16] {
        let screens = Screens::default();
        let fed: Vec<_> = (0..threads)
            .map(|t| screens.open(&format!("p{t}"), 120, 40))
            .collect();
        let passes = 6;
        let began = Instant::now();
        std::thread::scope(|scope| {
            for screen in &fed {
                let sample = sample.clone();
                scope.spawn(move || {
                    for _ in 0..passes {
                        for chunk in sample.chunks(8 * 1024) {
                            screen.feed(chunk);
                        }
                    }
                });
            }
        });
        let secs = began.elapsed().as_secs_f64();
        let mb = (threads * passes * sample.len()) as f64 / (1024.0 * 1024.0);
        println!(
            "screens  {threads:>2} panes fed at once   {:>7.1} MB/s together   {:>6.1} MB/s per pane",
            mb / secs,
            mb / secs / threads as f64
        );
        // A reader, while they print.
        let read_began = Instant::now();
        let mut read = 0usize;
        for _ in 0..200 {
            for t in 0..threads {
                read += screens
                    .read(&format!("p{t}"), |screen| screen.lines(100).len())
                    .unwrap_or(0);
            }
        }
        let per = read_began.elapsed().as_secs_f64() * 1e6 / (200 * threads) as f64;
        println!("screens  the last 100 lines of a pane read   {per:>7.1} us   ({read} lines)");
    }
}

struct Rig {
    host: EngineHost,
    shells: Arc<Shells>,
    panes: Vec<String>,
    file: std::path::PathBuf,
    _root: tempfile::TempDir,
}

fn rig(runtime: &tokio::runtime::Runtime) -> Rig {
    let workspaces: usize = option("--workspaces")
        .and_then(|n| n.parse().ok())
        .unwrap_or(8);
    let per: usize = option("--panes").and_then(|n| n.parse().ok()).unwrap_or(6);
    let root = tempfile::tempdir().unwrap();
    let mut panes = Vec::new();
    let state = AppState {
        workspaces: (0..workspaces)
            .map(|w| Workspace {
                id: format!("w{w}"),
                name: format!("Workspace {w}"),
                cwd: root.path().to_string_lossy().into_owned(),
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
    let file = root.path().join("node").join("state.json");
    hangar_engine::host::write_state_file(&file, &serde_json::to_value(&state).unwrap()).unwrap();
    let shells = Arc::new(Shells::default());
    let host = EngineHost::load(Config {
        core: hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(root.path())),
        terminals: shells.clone(),
        state_file: file.clone(),
    })
    .unwrap();
    let _guard = runtime.enter();
    host.start();
    let deadline = Instant::now() + Duration::from_secs(30);
    while shells.started() < panes.len() {
        assert!(Instant::now() < deadline, "the panes never started");
        std::thread::sleep(Duration::from_millis(10));
    }
    Rig {
        host,
        shells,
        panes,
        file,
        _root: root,
    }
}

fn bridge(pane: &str, event: serde_json::Value) -> hangar_core::Event {
    hangar_core::Event::BridgeEvent {
        pane_id: pane.to_string(),
        event,
    }
}

fn bench_bridge(rig: &Rig) {
    for pane in &rig.panes {
        rig.host.feed(bridge(
            pane,
            json!({ "kind": "hello", "sessionId": format!("s-{pane}"), "model": "opus", "version": "2.1" }),
        ));
    }
    // A turn of eight calls, per pane, round after round.
    let turn: Vec<serde_json::Value> = std::iter::once(json!({ "kind": "turn", "phase": "start" }))
        .chain((0..8).flat_map(|n| {
            [
                json!({ "kind": "tool", "phase": "start", "toolUseId": format!("t{n}"), "tool": "Bash",
                        "input": { "command": "cargo test --workspace" } }),
                json!({ "kind": "tool", "phase": "end", "toolUseId": format!("t{n}") }),
            ]
        }))
        .chain([
            json!({ "kind": "usage", "context": { "tokens": 52000, "window": 200000, "percent": 26.0 } }),
            json!({ "kind": "turn", "phase": "end", "reason": "answer", "durationMs": 4200, "answer": "Done." }),
        ])
        .collect();
    let rounds = 12;
    let cpu = cpu_ms();
    let began = Instant::now();
    let mut fed = 0usize;
    for _ in 0..rounds {
        for event in &turn {
            for pane in &rig.panes {
                rig.host.feed(bridge(pane, event.clone()));
                fed += 1;
            }
        }
    }
    let wall = began.elapsed();
    // What the background still owes these reports.
    std::thread::sleep(Duration::from_millis(600));
    let spent = cpu_ms() - cpu;
    println!(
        "bridge   {fed} reports over {} panes   {:>6.2} us each to take in   {:>5} ms of processor in all ({:.2} us each)",
        rig.panes.len(),
        wall.as_secs_f64() * 1e6 / fed as f64,
        spent,
        spent as f64 * 1e3 / fed as f64
    );
}

fn bench_output(rig: &Rig) {
    let threads = 4usize;
    let per_thread = 250_000usize;
    let cpu = cpu_ms();
    let began = Instant::now();
    std::thread::scope(|scope| {
        for t in 0..threads {
            let panes: Vec<(String, TermSink)> = rig
                .panes
                .iter()
                .skip(t)
                .step_by(threads)
                .filter_map(|pane| Some((pane.clone(), rig.shells.sink(pane)?)))
                .collect();
            scope.spawn(move || {
                for n in 0..per_thread {
                    let (pane, sink) = &panes[n % panes.len()];
                    sink(TermEvent::Output(pane.clone()));
                }
            });
        }
    });
    let wall = began.elapsed();
    std::thread::sleep(Duration::from_millis(600));
    let spent = cpu_ms() - cpu;
    let total = threads * per_thread;
    println!(
        "output   {total} chunks from {threads} reader threads   {:>6.0} ns each   {:>5} ms of processor in all",
        wall.as_secs_f64() * 1e9 / total as f64,
        spent
    );
}

fn bench_apply(rig: &Rig) {
    let writes = 2_000usize;
    let began = Instant::now();
    for n in 0..writes {
        let pane = &rig.panes[n % rig.panes.len()];
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
    println!(
        "apply    {writes} model writes   {:>6.1} us each",
        began.elapsed().as_secs_f64() * 1e6 / writes as f64
    );
}

fn bench_save(rig: &Rig, runtime: &tokio::runtime::Runtime) {
    let saves = 200usize;
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
    let size = std::fs::metadata(&rig.file).map(|m| m.len()).unwrap_or(0);
    println!(
        "save     {saves} saves of the state   {:>6.1} us each   {size} bytes on disk",
        began.elapsed().as_secs_f64() * 1e6 / saves as f64
    );
}

fn main() {
    if wanted("screens") {
        bench_screens();
    }
    let host_wanted = ["bridge", "output", "apply", "save"]
        .iter()
        .any(|name| wanted(name));
    if !host_wanted {
        return;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let rig = rig(&runtime);
    println!(
        "host     {} workspaces of {} panes",
        rig.host.snapshot().state.workspaces.len(),
        rig.panes.len() / rig.host.snapshot().state.workspaces.len().max(1)
    );
    if wanted("bridge") {
        bench_bridge(&rig);
    }
    if wanted("output") {
        bench_output(&rig);
    }
    if wanted("apply") {
        bench_apply(&rig);
    }
    if wanted("save") {
        bench_save(&rig, &runtime);
    }
    runtime.block_on(rig.host.shutdown());
}
