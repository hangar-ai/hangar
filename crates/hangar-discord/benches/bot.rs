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
//!   and how long after the last report;
//! - `person`: the person at their server while those sixteen agents keep
//!   working: a click on an agent's Stop and a message to it, forty times.
//!   How long until Discord has the click's acknowledgement (it shows
//!   "interaction failed" after 3 s), until the pane has the key, and until
//!   the agent has the message;
//! - `idle`: the connector beside 8, 50 and 200 workspaces where nothing
//!   happens: the processor it takes, and what it sends Discord (nothing).
//!
//! Options, after `--`: `--only answers|burst|person|idle`, `--agents N`,
//! `--turns N`, `--calls N`, `--rate N` (reports a second), `--latency MS`,
//! `--rounds N`.

#[path = "../tests/support/fake.rs"]
mod fake;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use hangar_discord::cards;
use hangar_discord::words::Words;
use hangar_engine::host::{Config, EngineHost, Spawn, TermSink, Terminals};
use hangar_engine::state::types::{AgentId, AppState, Pane, PaneRole, Settings, Workspace};
use hangar_i18n::Locale;

/// Shells that run nothing, and keep what was typed into them and when.
#[derive(Default)]
struct Shells {
    typed: Mutex<Vec<(String, String, Instant)>>,
}

impl Shells {
    /// When `pane` was first typed something `found` holds true for, past
    /// its first `skip` writes.
    fn typed_at(&self, pane: &str, skip: usize, found: impl Fn(&str) -> bool) -> Option<Instant> {
        self.typed
            .lock()
            .unwrap()
            .iter()
            .filter(|(to, _, _)| to == pane)
            .skip(skip)
            .find(|(_, data, _)| found(data))
            .map(|(_, _, at)| *at)
    }

    fn writes(&self, pane: &str) -> usize {
        self.typed
            .lock()
            .unwrap()
            .iter()
            .filter(|(to, _, _)| to == pane)
            .count()
    }
}

impl Terminals for Shells {
    fn spawn(&self, _spawn: Spawn, _sink: TermSink) -> Result<(), String> {
        Ok(())
    }
    fn write(&self, pane: &str, data: &str) -> Result<(), String> {
        self.typed
            .lock()
            .unwrap()
            .push((pane.to_string(), data.to_string(), Instant::now()));
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

/// A Hangar of `agents` workspaces, one orchestrator each, with its
/// connector on a fake Discord that takes `latency` to answer.
struct Rig {
    root: tempfile::TempDir,
    core: hangar_core::Host,
    host: EngineHost,
    shells: Arc<Shells>,
    discord: fake::Discord,
    dir: std::path::PathBuf,
    /// How long the server took to be made, and in how many calls.
    made: (Duration, usize),
}

async fn rig(agents: usize, latency: Duration) -> Rig {
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
    let shells = Arc::new(Shells::default());
    let host = EngineHost::load(Config {
        core: core.clone(),
        terminals: shells.clone(),
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

    // Every workspace has its channel and its dashboard before anything
    // is measured.
    let ready = Instant::now();
    while discord.calls_to("PUT", "/channels/").len() < agents {
        assert!(
            ready.elapsed() < Duration::from_secs(300),
            "the server was never made"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let made = (ready.elapsed(), discord.calls().len());
    Rig {
        root,
        core,
        host,
        shells,
        discord,
        dir,
        made,
    }
}

fn report(core: &hangar_core::Host, pane: String, event: serde_json::Value) {
    let _ = core.emit(hangar_core::Event::BridgeEvent {
        pane_id: pane,
        event,
    });
}

async fn bench_burst() {
    let agents = number("--agents", 16) as usize;
    let turns = number("--turns", 3) as usize;
    let calls = number("--calls", 30) as usize;
    let rate = number("--rate", 800);
    let latency = Duration::from_millis(number("--latency", 150));

    let rig = rig(agents, latency).await;
    let (core, host, discord) = (&rig.core, &rig.host, &rig.discord);
    let setup_calls = rig.made.1;
    println!(
        "burst    the server made for {agents} workspaces in {:.1} s ({setup_calls} calls)",
        rig.made.0.as_secs_f64()
    );

    let emit = |pane: String, event: serde_json::Value| report(core, pane, event);
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

/// `values`, sorted: its median, its 95th of a hundred, its largest.
fn spread(values: &mut [Duration]) -> String {
    values.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    match values {
        [] => "never".to_string(),
        _ => format!(
            "median {:6.1} ms   95th {:6.1} ms   slowest {:6.1} ms",
            ms(values[values.len() / 2]),
            ms(values[(values.len() * 95 / 100).min(values.len() - 1)]),
            ms(values[values.len() - 1]),
        ),
    }
}

/// Polls `found` every millisecond, for `patience` at most.
async fn seen<T>(patience: Duration, found: impl Fn() -> Option<T>) -> Option<T> {
    let began = Instant::now();
    loop {
        if let Some(value) = found() {
            return Some(value);
        }
        if began.elapsed() > patience {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

async fn bench_person() {
    let agents = number("--agents", 16) as usize;
    let calls = number("--calls", 30) as usize;
    let rate = number("--rate", 800);
    let rounds = number("--rounds", 40) as usize;
    let latency = Duration::from_millis(number("--latency", 150));

    let rig = rig(agents, latency).await;
    let discord = &rig.discord;
    let layout = hangar_discord::config::Layout::load(&rig.dir).unwrap();

    // The agents work for as long as the person is there: turn after turn
    // of tool calls, reported at `rate` a second.
    let stop = Arc::new(AtomicBool::new(false));
    let working = {
        let (core, stop) = (rig.core.clone(), stop.clone());
        std::thread::spawn(move || {
            for w in 0..agents {
                report(
                    &core,
                    format!("o{w}"),
                    json!({ "kind": "hello", "sessionId": format!("s{w}"), "model": "opus", "version": "2.1" }),
                );
            }
            let gap = Duration::from_micros(1_000_000 / rate.max(1));
            let mut next = Instant::now();
            let mut pace = || {
                next += gap;
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                }
            };
            let mut turn = 0usize;
            while !stop.load(Ordering::Relaxed) {
                for w in 0..agents {
                    report(
                        &core,
                        format!("o{w}"),
                        json!({ "kind": "turn", "phase": "start" }),
                    );
                    pace();
                }
                for c in 0..calls {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    for w in 0..agents {
                        report(
                            &core,
                            format!("o{w}"),
                            json!({ "kind": "tool", "phase": "start", "toolUseId": format!("t{turn}-{c}"),
                                    "tool": "Bash", "input": { "command": format!("cargo test step-{c}") } }),
                        );
                        pace();
                        report(
                            &core,
                            format!("o{w}"),
                            json!({ "kind": "tool", "phase": "end", "toolUseId": format!("t{turn}-{c}") }),
                        );
                        pace();
                    }
                }
                for w in 0..agents {
                    report(
                        &core,
                        format!("o{w}"),
                        json!({ "kind": "turn", "phase": "end", "reason": "answer", "durationMs": 1000,
                                "answer": format!("Answer of agent {w}, turn {turn}.") }),
                    );
                    pace();
                }
                turn += 1;
            }
        })
    };
    tokio::time::sleep(Duration::from_secs(2)).await;

    let (mut acked, mut stopped, mut given) = (Vec::new(), Vec::new(), Vec::new());
    for round in 0..rounds {
        let w = round % agents;
        let pane = format!("o{w}");
        let channel = layout.workspaces[&format!("w{w}")].channel.clone();

        // A click on the agent's Stop.
        let id = format!("i{round}");
        let before = rig.shells.writes(&pane);
        let clicked = Instant::now();
        discord.send(
            "INTERACTION_CREATE",
            json!({ "id": id, "token": format!("{id}tok"), "type": 3, "channel_id": channel,
                    "member": { "user": { "id": "u1" } },
                    "data": { "custom_id": format!("s:{pane}") }, "message": { "id": "1" } }),
        );
        let callback = format!("/interactions/{id}/{id}tok/callback");
        let patience = Duration::from_secs(10);
        if let Some(at) = seen(patience, || {
            discord.calls_to("POST", &callback).first().map(|c| c.at)
        })
        .await
        {
            acked.push(at.saturating_duration_since(clicked));
        }
        // An agent between two turns has nothing to stop: no key then.
        let escape = |data: &str| data.contains('\u{1b}');
        if let Some(at) = seen(Duration::from_secs(1), || {
            rig.shells.typed_at(&pane, before, escape)
        })
        .await
        {
            stopped.push(at.saturating_duration_since(clicked));
        }

        // A message to the agent: typed into its pane, or queued for its
        // turn's end (the hourglass on the message).
        let (message, text) = (format!("m{round}"), format!("note {round} for agent {w}"));
        let before = rig.shells.writes(&pane);
        let wrote = Instant::now();
        discord.send(
            "MESSAGE_CREATE",
            json!({ "id": message, "channel_id": channel, "content": text,
                    "author": { "id": "u1" } }),
        );
        let reaction = format!("/channels/{channel}/messages/{message}/reactions/");
        let handed = seen(patience, || {
            rig.shells
                .typed_at(&pane, before, |data| data.contains(&text))
                .or_else(|| discord.calls_to("PUT", &reaction).first().map(|c| c.at))
        })
        .await;
        if let Some(at) = handed {
            given.push(at.saturating_duration_since(wrote));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stop.store(true, Ordering::Relaxed);
    let _ = working.join();

    println!(
        "person   {agents} agents at work ({rate} reports a second), Discord answering in {} ms; {rounds} clicks and {rounds} messages",
        latency.as_millis()
    );
    println!(
        "person   a click acknowledged at Discord   {}/{rounds}   {}",
        acked.len(),
        spread(&mut acked)
    );
    println!(
        "person   Escape in the agent's pane        {}/{rounds}   {}",
        stopped.len(),
        spread(&mut stopped)
    );
    println!(
        "person   a message handed to its agent     {}/{rounds}   {}",
        given.len(),
        spread(&mut given)
    );
    rig.host.shutdown().await;
}

/// This process's processor time, in seconds (`/proc/self/stat`).
fn cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .map(|(_, rest)| rest.split_whitespace().collect())
        .unwrap_or_default();
    let ticks = |n: usize| {
        fields
            .get(n)
            .and_then(|f| f.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    (ticks(11) + ticks(12)) / 100.0
}

async fn bench_idle() {
    for workspaces in [8usize, 50, 200] {
        let rig = rig(workspaces, Duration::ZERO).await;
        // What the making left to settle.
        tokio::time::sleep(Duration::from_secs(8)).await;
        let (calls, cpu, began) = (rig.discord.calls().len(), cpu_seconds(), Instant::now());
        tokio::time::sleep(Duration::from_secs(20)).await;
        let seconds = began.elapsed().as_secs_f64();
        let spent = cpu_seconds() - cpu;
        println!(
            "idle     {workspaces:>3} workspaces where nothing happens   {:5.2} ms of processor a second ({:.2} % of one)   {} calls to Discord in {seconds:.0} s",
            spent * 1000.0 / seconds,
            spent * 100.0 / seconds,
            rig.discord.calls().len() - calls,
        );
        rig.host.shutdown().await;
        drop(rig.root);
    }
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
    for (name, bench) in [("person", 0), ("idle", 1)] {
        if !wanted(name) {
            continue;
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();
        if bench == 0 {
            runtime.block_on(bench_person());
        } else {
            runtime.block_on(bench_idle());
        }
    }
}
