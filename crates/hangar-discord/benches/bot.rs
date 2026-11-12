//! What the connector does under load: `cargo bench -p hangar-discord
//! --bench bot`.
//!
//! - `answers`: a long answer of Claude's cut into the pieces Discord takes
//!   (MB/s), and a turn's card built;
//! - `burst`: sixteen agents working at once, each through three turns of
//!   thirty tool calls, their mods' reports arriving at 800 a second, while
//!   Discord (a fake one, `tests/support/fake.rs`) takes 150 ms to answer
//!   each call, as it does from a server far from it. What matters is what
//!   reached the server: every turn's card folded and every answer posted,
//!   and how long after the last report.
//!
//! Options, after `--`: `--only answers|burst`, `--agents N`, `--turns N`,
//! `--calls N`, `--rate N` (reports a second), `--latency MS`.

#[path = "../tests/support/fake.rs"]
mod fake;

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;

use hangar_discord::cards;
use hangar_discord::words::Words;
use hangar_engine::host::{Config, EngineHost, Spawn, TermSink, Terminals};
use hangar_engine::state::types::{AgentId, AppState, Pane, PaneRole, Settings, Workspace};
use hangar_i18n::Locale;

struct Shells;

impl Terminals for Shells {
    fn spawn(&self, _spawn: Spawn, _sink: TermSink) -> Result<(), String> {
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

fn number(name: &str, default: u64) -> u64 {
    option(name).and_then(|n| n.parse().ok()).unwrap_or(default)
}

fn wanted(name: &str) -> bool {
    option("--only").is_none_or(|only| only == name)
}

fn bench_answers() {
    let mut answer = String::new();
    let mut n = 0;
    while answer.len() < 400 * 1024 {
        n += 1;
        answer.push_str(&format!(
            "## Step {n}\n\nThe index is rebuilt from the journal, then swapped in: readers never see half of it.\n\n```rust\nfn step_{n}(index: &mut Index) -> Result<(), Error> {{\n    index.rebuild()?;\n    index.swap()\n}}\n```\n\n"
        ));
    }
    let passes = 20;
    let began = Instant::now();
    let mut pieces = 0;
    for _ in 0..passes {
        pieces = cards::split_answer(&answer).len();
    }
    let secs = began.elapsed().as_secs_f64();
    println!(
        "answers  a {} KiB answer cut into {pieces} pieces   {:>7.1} MB/s",
        answer.len() / 1024,
        (passes * answer.len()) as f64 / (1024.0 * 1024.0) / secs
    );
    let words = Words::of(Some(Locale::En));
    let calls: Vec<String> = (0..40)
        .map(|n| format!("Bash cargo test -p crate-{n}"))
        .collect();
    let cards_built = 50_000;
    let began = Instant::now();
    let mut bytes = 0;
    for _ in 0..cards_built {
        bytes += cards::turn(words, "Leo", 1_800_000_000, &calls, "pane")
            .to_string()
            .len();
    }
    println!(
        "answers  a turn's card built and written   {:>7.2} us   ({} bytes)",
        began.elapsed().as_secs_f64() * 1e6 / cards_built as f64,
        bytes / cards_built
    );
}

async fn bench_burst() {
    let agents = number("--agents", 16) as usize;
    let turns = number("--turns", 3) as usize;
    let calls = number("--calls", 30) as usize;
    let rate = number("--rate", 800);
    let latency = Duration::from_millis(number("--latency", 150));

    let root = tempfile::tempdir().unwrap();
    let state = AppState {
        workspaces: (0..agents)
            .map(|w| Workspace {
                id: format!("w{w}"),
                name: format!("Project {w}"),
                cwd: root.path().to_string_lossy().into_owned(),
                panes: vec![Pane {
                    id: format!("o{w}"),
                    name: format!("Agent {w}"),
                    agent: AgentId::Claude,
                    role: Some(PaneRole::Orchestrator),
                    ..Pane::default()
                }],
                ..Workspace::default()
            })
            .collect(),
        settings: Settings {
            claude_bridge: true,
            locale: Some(Locale::En),
            ..Settings::default()
        },
        ..AppState::default()
    };
    let dir = root.path().join("node");
    let file = dir.join("state.json");
    hangar_engine::host::write_state_file(&file, &serde_json::to_value(&state).unwrap()).unwrap();
    let core = hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(root.path()));
    let host = EngineHost::load(Config {
        core: core.clone(),
        terminals: Arc::new(Shells),
        state_file: file,
    })
    .unwrap();
    host.start();
    let discord = fake::Discord::start_on(0, latency).await;
    hangar_discord::Settings {
        token: "secret".into(),
        guild: "g1".into(),
        owner: Some("u1".into()),
        ..hangar_discord::Settings::default()
    }
    .save(&dir)
    .unwrap();
    let options = hangar_discord::Options {
        api: discord.api(),
        gateway: Some(discord.gateway_url()),
        dir: dir.clone(),
    };
    tokio::spawn(hangar_discord::run(Arc::new(host.clone()), options));

    // Every workspace has its channel and its dashboard before the burst.
    let ready = Instant::now();
    while discord.calls_to("PUT", "/channels/").len() < agents {
        assert!(
            ready.elapsed() < Duration::from_secs(120),
            "the server was never made"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let setup_calls = discord.calls().len();
    println!(
        "burst    the server made for {agents} workspaces in {:.1} s ({setup_calls} calls)",
        ready.elapsed().as_secs_f64()
    );

    let emit = |pane: String, event: serde_json::Value| {
        let _ = core.emit(hangar_core::Event::BridgeEvent {
            pane_id: pane,
            event,
        });
    };
    for w in 0..agents {
        emit(
            format!("o{w}"),
            json!({ "kind": "hello", "sessionId": format!("s{w}"), "model": "opus", "version": "2.1" }),
        );
    }
    let gap = Duration::from_micros(1_000_000 / rate.max(1));
    let began = Instant::now();
    let mut reports = 0usize;
    let mut next = Instant::now();
    let mut pace = |reports: &mut usize| {
        *reports += 1;
        next += gap;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
    };
    for t in 0..turns {
        for w in 0..agents {
            emit(format!("o{w}"), json!({ "kind": "turn", "phase": "start" }));
            pace(&mut reports);
        }
        for c in 0..calls {
            for w in 0..agents {
                emit(
                    format!("o{w}"),
                    json!({ "kind": "tool", "phase": "start", "toolUseId": format!("t{t}-{c}"),
                            "tool": "Bash", "input": { "command": format!("cargo test step-{c}") } }),
                );
                pace(&mut reports);
                emit(
                    format!("o{w}"),
                    json!({ "kind": "tool", "phase": "end", "toolUseId": format!("t{t}-{c}") }),
                );
                pace(&mut reports);
            }
        }
        for w in 0..agents {
            emit(
                format!("o{w}"),
                json!({ "kind": "turn", "phase": "end", "reason": "answer", "durationMs": 1000,
                        "answer": format!("Answer of agent {w}, turn {t}.") }),
            );
            pace(&mut reports);
        }
    }
    let emitted = began.elapsed();
    let expected = agents * turns;
    let answers = || discord.count("POST", "Answer of agent");
    let folded = || discord.count("PATCH", "actions ·");
    // Until everything reached the server, or nothing more does.
    let after = Instant::now();
    let mut last = (0, Instant::now());
    loop {
        let calls_now = discord.calls().len();
        if calls_now != last.0 {
            last = (calls_now, Instant::now());
        }
        let done = answers() >= expected && folded() >= expected;
        if done
            || last.1.elapsed() > Duration::from_secs(8)
            || after.elapsed() > Duration::from_secs(180)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let drained = if answers() >= expected && folded() >= expected {
        format!(
            "{:.1} s after the last report",
            after.elapsed().as_secs_f64()
        )
    } else {
        "never: reports were lost".to_string()
    };
    println!(
        "burst    {agents} agents, {turns} turns of {calls} calls ({reports} reports in {:.1} s), Discord answering in {} ms",
        emitted.as_secs_f64(),
        latency.as_millis()
    );
    println!(
        "burst    answers posted {}/{expected}   turns folded {}/{expected}   all there {drained}   {} calls",
        answers(),
        folded(),
        discord.calls().len() - setup_calls
    );
    host.shutdown().await;
}

fn main() {
    if wanted("answers") {
        bench_answers();
    }
    if wanted("burst") {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(bench_burst());
    }
}
