//! The connector against a Discord of the test's own: its REST routes
//! record every call and answer as Discord would, its gateway says hello,
//! hears the bot identify, and hands it the events the test makes up. The
//! engine runs with no window over shells that only record what they are
//! typed.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

use hangar_engine::control::port::ModelCmd;
use hangar_engine::host::{Config, EngineHost, Spawn, TermEvent, TermSink, Terminals};
use hangar_engine::orchestration::events::{DeliveryOutcome, OrchestratorEvent};
use hangar_engine::state::types::{AgentId, AppState, Pane, PaneRole, Settings, Workspace};
use hangar_i18n::Locale;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/* ── shells that record ─────────────────────────────────────────── */

#[derive(Default)]
struct Shells {
    alive: Mutex<HashSet<String>>,
    typed: Mutex<Vec<(String, String)>>,
}

impl Shells {
    fn typed_into(&self, pane: &str) -> String {
        locked(&self.typed)
            .iter()
            .filter(|(id, _)| id == pane)
            .map(|(_, text)| text.as_str())
            .collect()
    }
}

impl Terminals for Shells {
    fn spawn(&self, spawn: Spawn, sink: TermSink) -> Result<(), String> {
        locked(&self.alive).insert(spawn.id.clone());
        sink(TermEvent::Output(spawn.id));
        Ok(())
    }
    fn write(&self, pane: &str, data: &str) -> Result<(), String> {
        locked(&self.typed).push((pane.to_string(), data.to_string()));
        Ok(())
    }
    fn alive(&self, pane: &str) -> bool {
        locked(&self.alive).contains(pane)
    }
    fn kill(&self, pane: &str) -> Result<(), String> {
        locked(&self.alive).remove(pane);
        Ok(())
    }
    fn lines(&self, pane: &str, _n: usize) -> Option<Vec<String>> {
        self.alive(pane).then(Vec::new)
    }
    fn screen(&self, pane: &str) -> Option<Vec<String>> {
        self.alive(pane).then(Vec::new)
    }
    fn kill_all(&self) {
        locked(&self.alive).clear();
    }
}

/* ── a Discord of the test's own ────────────────────────────────── */

#[path = "support/fake.rs"]
mod fake;
use fake::Discord;

/// Waits up to twenty seconds for `found` (a loaded machine is slow).
async fn eventually<T>(what: &str, mut found: impl FnMut() -> Option<T>) -> T {
    for _ in 0..2_000 {
        if let Some(found) = found() {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never happened: {what}");
}

fn pane(id: &str, name: &str, role: Option<PaneRole>) -> Pane {
    Pane {
        id: id.into(),
        name: name.into(),
        agent: AgentId::Claude,
        role,
        ..Pane::default()
    }
}

struct Rig {
    host: EngineHost,
    shells: Arc<Shells>,
    discord: Discord,
    dir: PathBuf,
    _root: tempfile::TempDir,
}

async fn rig() -> Rig {
    let root = tempfile::tempdir().unwrap();
    let state = AppState {
        workspaces: vec![Workspace {
            id: "shop".into(),
            name: "My Shop".into(),
            cwd: root.path().to_string_lossy().into_owned(),
            panes: vec![pane("leo", "Leo", Some(PaneRole::Orchestrator))],
            ..Workspace::default()
        }],
        settings: Settings {
            locale: Some(Locale::Fr),
            ..Settings::default()
        },
        ..AppState::default()
    };
    let dir = root.path().join("node");
    let file = dir.join("state.json");
    hangar_engine::host::write_state_file(&file, &serde_json::to_value(&state).unwrap()).unwrap();
    let shells = Arc::new(Shells::default());
    let host = EngineHost::load(Config {
        core: hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(root.path())),
        terminals: shells.clone(),
        state_file: file,
    })
    .unwrap();
    host.start();
    let discord = Discord::start().await;
    hangar_discord::Settings {
        token: "secret".into(),
        guild: "g1".into(),
        ..hangar_discord::Settings::default()
    }
    .save(&dir)
    .unwrap();
    let options = hangar_discord::Options {
        api: discord.api(),
        gateway: Some(discord.gateway_url()),
        dir: dir.clone(),
    };
    let bot_host = host.clone();
    tokio::spawn(async move {
        if let Err(error) = hangar_discord::run(Arc::new(bot_host), options).await {
            panic!("the connector stopped: {error}");
        }
    });
    Rig {
        host,
        shells,
        discord,
        dir,
        _root: root,
    }
}

fn interaction(
    id: &str,
    user: &str,
    channel: &str,
    kind: u8,
    data: Value,
    message: Option<&str>,
) -> Value {
    let mut i = json!({
        "id": id, "token": format!("{id}tok"), "type": kind, "channel_id": channel,
        "member": { "user": { "id": user } }, "data": data,
    });
    if let Some(message) = message {
        i["message"] = json!({ "id": message });
    }
    i
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_on_discord_from_pairing_to_a_worker_s_thread() {
    let rig = rig().await;
    let d = &rig.discord;

    // The bot identifies with its token and the intents it needs.
    let identified = eventually("identify", || d.identified()).await;
    assert_eq!(identified["token"], "secret");
    assert_eq!(identified["intents"].as_u64().unwrap() & (1 << 15), 1 << 15);
    // Its commands, on the guild alone.
    eventually("the commands registered", || {
        d.calls_to("PUT", "/applications/app1/guilds/g1/commands")
            .pop()
    })
    .await;

    // A category, the workspace's channel, its webhook, its dashboard pinned.
    let made = eventually("the workspace's channel", || {
        d.calls_to("POST", "/guilds/g1/channels")
            .into_iter()
            .find(|c| c.body["type"] == 0)
    })
    .await;
    assert_eq!(made.body["name"], "my-shop");
    let category = d.calls_to("POST", "/guilds/g1/channels")[0].clone();
    assert_eq!(category.body["type"], 4);
    assert_eq!(category.body["permission_overwrites"][0]["id"], "bot1");
    let layout: Value = eventually("the layout saved", || {
        std::fs::read_to_string(rig.dir.join("discord-layout.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .filter(|layout| layout["workspaces"]["shop"]["dashboard"].is_string())
    })
    .await;
    let channel = layout["workspaces"]["shop"]["channel"]
        .as_str()
        .unwrap()
        .to_string();
    let hook = layout["workspaces"]["shop"]["webhook"][0]
        .as_str()
        .unwrap()
        .to_string();
    eventually("the dashboard pinned", || {
        d.calls_to("PUT", &format!("/channels/{channel}/messages/pins/"))
            .pop()
    })
    .await;

    // Not paired yet: the code in the settings pairs whoever sends it.
    let settings: Value =
        serde_json::from_str(&std::fs::read_to_string(rig.dir.join("discord.json")).unwrap())
            .unwrap();
    let code = settings["pairing"].as_str().unwrap().to_string();
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m1", "channel_id": channel, "content": "hello",
        "author": { "id": "u1" } }),
    );
    eventually("told to pair", || d.find("POST", "pair <code>")).await;
    // The code is heard on the server alone: not in a direct message, not
    // on another server the bot was invited to.
    for (id, guild) in [("dm", Value::Null), ("elsewhere", json!("g2"))] {
        d.send(
            "MESSAGE_CREATE",
            json!({ "id": id, "channel_id": channel, "guild_id": guild,
            "content": format!("pair {code}"), "author": { "id": "intruder" } }),
        );
    }
    // A wrong code is told wrong, five times; after that its sender is
    // not heard any more, even with the right one.
    for n in 0..6 {
        d.send(
            "MESSAGE_CREATE",
            json!({ "id": format!("w{n}"), "channel_id": channel, "guild_id": "g1",
            "content": "pair 000000x", "author": { "id": "guesser" } }),
        );
    }
    eventually("five wrong codes told so", || {
        (d.count("POST", "Ce n'est pas le code") == 5).then_some(())
    })
    .await;
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "w6", "channel_id": channel, "guild_id": "g1",
        "content": format!("pair {code}"), "author": { "id": "guesser" } }),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let unpaired = std::fs::read_to_string(rig.dir.join("discord.json")).unwrap();
    assert!(unpaired.contains("\"owner\": null"), "{unpaired}");
    assert_eq!(d.count("POST", "Ce n'est pas le code"), 5);
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m2", "channel_id": channel, "guild_id": "g1",
        "content": format!("pair {code}"), "author": { "id": "u1" } }),
    );
    eventually("paired", || {
        std::fs::read_to_string(rig.dir.join("discord.json"))
            .ok()
            .filter(|raw| raw.contains("\"owner\": \"u1\""))
    })
    .await;

    // What the owner writes in the channel reaches the orchestrator.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m3", "channel_id": channel,
        "content": "Ajoute un panier", "author": { "id": "u1" } }),
    );
    // Typed when Leo is idle, queued (⏳) while it works.
    eventually("Leo given the message", || {
        let queued = !d
            .calls_to(
                "PUT",
                &format!("/channels/{channel}/messages/m3/reactions/%E2%8F%B3"),
            )
            .is_empty();
        (queued || rig.shells.typed_into("leo").contains("Ajoute un panier")).then_some(())
    })
    .await;
    eventually("the message marked", || {
        d.calls_to(
            "PUT",
            &format!("/channels/{channel}/messages/m3/reactions/"),
        )
        .pop()
    })
    .await;
    // Messages sent one after the other reach the agent in that order.
    for (n, word) in ["premier", "deuxième", "troisième", "quatrième"]
        .iter()
        .enumerate()
    {
        d.send(
            "MESSAGE_CREATE",
            json!({ "id": format!("o{n}"), "channel_id": channel, "content": word,
            "author": { "id": "u1" } }),
        );
    }
    eventually("the four typed", || {
        rig.shells
            .typed_into("leo")
            .contains("quatrième")
            .then_some(())
    })
    .await;
    let typed = rig.shells.typed_into("leo");
    let at = |word: &str| typed.find(word).expect(word);
    assert!(
        at("premier") < at("deuxième")
            && at("deuxième") < at("troisième")
            && at("troisième") < at("quatrième"),
        "{typed:?}"
    );
    // Anyone else is not heard.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m4", "channel_id": channel,
        "content": "rm -rf /", "author": { "id": "intruder" } }),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!rig.shells.typed_into("leo").contains("rm -rf /"));

    // A turn of Leo's: its card under Leo's name, its calls, then folded.
    let core = rig.host.core().clone();
    let bridge = |event: Value| {
        core.emit(hangar_core::Event::BridgeEvent {
            pane_id: "leo".into(),
            event,
        })
        .unwrap();
    };
    bridge(json!({ "kind": "hello", "sessionId": "s1", "model": null, "version": "2.1" }));
    bridge(json!({ "kind": "turn", "phase": "start" }));
    let card = eventually("the turn's card", || d.find("POST", "travaille")).await;
    assert!(card.path.starts_with(&format!("/webhooks/{hook}/")));
    assert!(card.path.contains("with_components=true"));
    assert_eq!(card.body["username"], "Leo");
    assert!(
        card.body["avatar_url"]
            .as_str()
            .is_some_and(|url| url.starts_with("https://cdn.discordapp.com/embed/avatars/"))
    );

    // While Leo works, what the owner writes waits in its queue (⏳); an
    // edit changes what waits, a delete takes it back.
    eventually("Leo working", || {
        rig.host
            .snapshot()
            .activity
            .get("leo")
            .is_some_and(|a| a.activity == hangar_engine::state::AgentActivity::Working)
            .then_some(())
    })
    .await;
    let queued = |text: &str| {
        hangar_core::bridge::bridge_queue("leo".into())
            .iter()
            .any(|entry| entry.text == text)
    };
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "q1", "channel_id": channel, "content": "Et le paiement ?",
        "author": { "id": "u1" } }),
    );
    eventually("queued, marked ⏳", || {
        (queued("Et le paiement ?")
            && !d
                .calls_to(
                    "PUT",
                    &format!("/channels/{channel}/messages/q1/reactions/%E2%8F%B3"),
                )
                .is_empty())
        .then_some(())
    })
    .await;
    d.send(
        "MESSAGE_UPDATE",
        json!({ "id": "q1", "channel_id": channel, "content": "Et le paiement Stripe ?",
        "author": { "id": "u1" } }),
    );
    eventually("the edit waits instead", || {
        (queued("Et le paiement Stripe ?") && !queued("Et le paiement ?")).then_some(())
    })
    .await;
    d.send(
        "MESSAGE_DELETE",
        json!({ "id": "q1", "channel_id": channel }),
    );
    eventually("taken back", || {
        (!queued("Et le paiement Stripe ?")).then_some(())
    })
    .await;
    bridge(
        json!({ "kind": "tool", "phase": "start", "toolUseId": "t1", "tool": "Bash",
        "input": { "command": "npm test" } }),
    );
    eventually("the card edited with the call", || {
        d.find("PATCH", "Bash npm test")
    })
    .await;
    bridge(
        json!({ "kind": "turn", "phase": "end", "reason": "answer", "durationMs": 1000,
        "answer": "Le panier est prêt." }),
    );
    eventually("the turn folded", || d.find("PATCH", "1 action")).await;
    let answer = eventually("the answer", || d.find("POST", "Le panier est prêt.")).await;
    assert_eq!(answer.body["username"], "Leo");

    // A permission: a card that calls the owner, Allow answers it.
    bridge(
        json!({ "kind": "permission", "waiting": true, "tool": "Bash",
        "input": { "command": "rm -rf build" } }),
    );
    let asked = eventually("the permission's card", || d.find("POST", "p:leo:y")).await;
    assert!(asked.body.to_string().contains("<@u1>"));
    assert_eq!(asked.body["allowed_mentions"]["users"], json!(["u1"]));
    // A stranger's click is refused, to them alone.
    d.send(
        "INTERACTION_CREATE",
        interaction(
            "i0",
            "intruder",
            &channel,
            3,
            json!({ "custom_id": "p:leo:y" }),
            None,
        ),
    );
    let refused = eventually("the stranger refused", || {
        d.calls_to("POST", "/interactions/i0/i0tok/callback").pop()
    })
    .await;
    assert_eq!(refused.body["type"], 4);
    assert_eq!(refused.body["data"]["flags"], 64);
    let typed_before = rig.shells.typed_into("leo").len();
    // While Leo waits on that answer, what the owner writes is not typed:
    // it would land in the question, and its Enter would say yes.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m5", "channel_id": channel, "guild_id": "g1",
        "content": "attends, pourquoi ?", "author": { "id": "u1" } }),
    );
    eventually("told to answer the card first", || {
        d.find("POST", "attend une réponse")
    })
    .await;
    assert_eq!(rig.shells.typed_into("leo").len(), typed_before);
    // A card left from an earlier question (answered on the PC, or posted
    // before Hangar restarted) answers nothing: clicked, it says so, and
    // what Leo asks now is not allowed by it.
    d.send(
        "INTERACTION_CREATE",
        interaction(
            "iold",
            "u1",
            &channel,
            3,
            json!({ "custom_id": "p:leo:y" }),
            Some("999"),
        ),
    );
    let stale = eventually("the old card says it is answered", || {
        d.calls_to("PATCH", "/webhooks/app1/ioldtok/messages/@original")
            .pop()
    })
    .await;
    assert!(
        stale.body.to_string().contains("Répondu ailleurs"),
        "{stale:?}"
    );
    assert_eq!(
        d.calls_to("POST", "/interactions/iold/ioldtok/callback")[0].body["type"],
        6
    );
    assert_eq!(rig.shells.typed_into("leo").len(), typed_before);
    assert!(d.find("PATCH", "Autorisé").is_none());
    // The card of what Leo asks now does answer it.
    d.send(
        "INTERACTION_CREATE",
        interaction(
            "i1",
            "u1",
            &channel,
            3,
            json!({ "custom_id": "p:leo:y" }),
            Some(&asked.id),
        ),
    );
    let acked = eventually("the click acknowledged", || {
        d.calls_to("POST", "/interactions/i1/i1tok/callback").pop()
    })
    .await;
    assert_eq!(acked.body["type"], 6);
    eventually("the answer typed or handed to the mod", || {
        (rig.shells.typed_into("leo").len() > typed_before || d.find("PATCH", "Autorisé").is_some())
            .then_some(())
    })
    .await;

    // A worker started: its line, with the button that opens its thread.
    let added = rig
        .host
        .apply(ModelCmd::AddPane {
            ws: "shop".into(),
            agent: Some(AgentId::Claude),
            near: None,
            dir: None,
            cwd: None,
            name: Some("Ada".into()),
            init: Some(hangar_engine::state::PanePatch::new(|p: &mut Pane| {
                p.role = Some(PaneRole::Worker);
                p.task_id = Some("t1".into());
            })),
        })
        .unwrap();
    let ada = added.pane.unwrap();
    rig.host.orchestration().unwrap().post(
        "shop",
        OrchestratorEvent::WorkerStarted {
            worker: "Ada".into(),
            task_id: Some("t1".into()),
        },
    );
    let line = eventually("the worker's line", || d.find("POST", &format!("t:{ada}"))).await;
    assert!(line.body.to_string().contains("Ouvrir le fil"));
    let line_id = eventually("its id kept", || {
        std::fs::read_to_string(rig.dir.join("discord-layout.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|layout| layout["workers"][&ada]["line"].as_str().map(str::to_string))
    })
    .await;
    d.send(
        "INTERACTION_CREATE",
        interaction(
            "i2",
            "u1",
            &channel,
            3,
            json!({ "custom_id": format!("t:{ada}") }),
            Some(&line_id),
        ),
    );
    let thread = eventually("the thread started on the line", || {
        d.calls_to(
            "POST",
            &format!("/channels/{channel}/messages/{line_id}/threads"),
        )
        .pop()
    })
    .await;
    assert_eq!(thread.body["auto_archive_duration"], 10080);
    assert!(thread.body["name"].as_str().unwrap().starts_with("Ada"));
    eventually("the line links to the thread", || {
        d.calls_to("PATCH", &format!("/channels/{channel}/messages/{line_id}"))
            .into_iter()
            .find(|c| c.body.to_string().contains("Voir le fil"))
    })
    .await;
    let thread_id = eventually("the thread kept", || {
        std::fs::read_to_string(rig.dir.join("discord-layout.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|layout| {
                layout["workers"][&ada]["thread"]
                    .as_str()
                    .map(str::to_string)
            })
    })
    .await;
    // What the owner writes in the thread goes to the worker.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "m5", "channel_id": thread_id,
        "content": "Pense aux tests", "author": { "id": "u1" } }),
    );
    eventually("Ada given the message", || {
        let queued = !d
            .calls_to(
                "PUT",
                &format!("/channels/{thread_id}/messages/m5/reactions/%E2%8F%B3"),
            )
            .is_empty();
        (queued || rig.shells.typed_into(&ada).contains("Pense aux tests")).then_some(())
    })
    .await;
    // The worker stops: a last line, the thread archived.
    rig.host.orchestration().unwrap().post(
        "shop",
        OrchestratorEvent::WorkerExited {
            worker: "Ada".into(),
            task_id: Some("t1".into()),
        },
    );
    eventually("the thread archived", || {
        d.calls_to("PATCH", &format!("/channels/{thread_id}"))
            .into_iter()
            .find(|c| c.body["archived"] == true)
    })
    .await;

    // Ada's delivery passed its check and review: the journal says so,
    // and the card that calls the owner has Land and Send back. Landed,
    // the card says so in place of its buttons; ready again later, it is
    // a card of its own.
    let delivery = |outcome| OrchestratorEvent::Delivery {
        task_id: "t1".into(),
        title: "Le panier".into(),
        worker: "Ada".into(),
        outcome,
        detail: String::new(),
    };
    let orchestration = rig.host.orchestration().unwrap();
    orchestration.post("shop", delivery(DeliveryOutcome::Ready));
    let ready = eventually("the delivery's card", || d.find("POST", "d:y:shop:t1")).await;
    assert!(
        ready.body.to_string().contains("Livraison prête"),
        "{ready:?}"
    );
    assert!(ready.body.to_string().contains("Le panier"));
    assert!(ready.body.to_string().contains("d:n:shop:t1"));
    assert_eq!(ready.body["allowed_mentions"]["users"], json!(["u1"]));
    orchestration.post("shop", delivery(DeliveryOutcome::Landed));
    let landed = eventually("the card says landed", || {
        d.calls_to(
            "PATCH",
            &format!("/channels/{channel}/messages/{}", ready.id),
        )
        .pop()
    })
    .await;
    assert!(landed.body.to_string().contains("Fusionné"), "{landed:?}");
    orchestration.post("shop", delivery(DeliveryOutcome::Ready));
    eventually("a second delivery, a second card", || {
        (d.count("POST", "d:y:shop:t1") == 2).then_some(())
    })
    .await;

    // A workspace made from Discord: its channel follows.
    let folder = rig._root.path().join("blog");
    std::fs::create_dir_all(&folder).unwrap();
    d.send("INTERACTION_CREATE", interaction("i3", "u1", &channel, 2, json!({
        "name": "workspace",
        "options": [{ "name": "folder", "value": folder.to_string_lossy() }, { "name": "name", "value": "Blog" }],
    }), None));
    eventually("the blog's channel", || {
        d.calls_to("POST", "/guilds/g1/channels")
            .into_iter()
            .find(|c| c.body["name"] == "blog")
    })
    .await;
    let said = eventually("the command answered", || d.find("PATCH", "Workspace créé")).await;
    assert!(
        said.path
            .starts_with("/webhooks/app1/i3tok/messages/@original")
    );
    assert!(
        rig.host
            .snapshot()
            .state
            .workspaces
            .iter()
            .any(|ws| ws.name == "Blog")
    );
}

/// A Hangar that shares its bot: whether it leads, and what the others
/// made.
struct Sharing {
    host: EngineHost,
    leads: bool,
    others: Arc<Mutex<HashSet<String>>>,
}

impl hangar_discord::Hangar for Sharing {
    fn port(&self) -> hangar_engine::control::port::Port {
        self.host.port()
    }
    fn desk(&self) -> Option<Arc<hangar_engine::mobile::engine::Mobile>> {
        self.host.desk()
    }
    fn journal(&self, ws: &str) -> Vec<hangar_engine::orchestration::events::JournalEntry> {
        hangar_discord::Hangar::journal(&self.host, ws)
    }
    fn tasks(&self, ws: &str) -> Vec<hangar_core::board::Task> {
        hangar_discord::Hangar::tasks(&self.host, ws)
    }
    fn events(&self) -> tokio::sync::broadcast::Receiver<hangar_core::Event> {
        self.host.core().subscribe()
    }
    fn leads(&self) -> bool {
        self.leads
    }
    fn others(&self) -> HashSet<String> {
        locked(&self.others).clone()
    }
}

struct Side {
    host: EngineHost,
    shells: Arc<Shells>,
    dir: PathBuf,
}

/// One Hangar with workspace `ws` (its orchestrator `pane_id`), its bot on
/// `d`.
fn side(
    root: &std::path::Path,
    name: &str,
    (ws, pane_id): (&str, &str),
    settings: hangar_discord::Settings,
    leads: bool,
    others: Arc<Mutex<HashSet<String>>>,
    d: &Discord,
) -> Side {
    let folder = root.join(name);
    std::fs::create_dir_all(&folder).unwrap();
    let state = AppState {
        workspaces: vec![Workspace {
            id: ws.into(),
            name: ws.into(),
            cwd: folder.to_string_lossy().into_owned(),
            panes: vec![pane(pane_id, "Orchestrator", Some(PaneRole::Orchestrator))],
            ..Workspace::default()
        }],
        ..AppState::default()
    };
    let dir = folder.join("hangar");
    let file = dir.join("state.json");
    hangar_engine::host::write_state_file(&file, &serde_json::to_value(&state).unwrap()).unwrap();
    let shells = Arc::new(Shells::default());
    let host = EngineHost::load(Config {
        core: hangar_core::Host::with_paths(hangar_core::paths::Paths::rooted(&folder)),
        terminals: shells.clone(),
        state_file: file,
    })
    .unwrap();
    host.start();
    settings.save(&dir).unwrap();
    let options = hangar_discord::Options {
        api: d.api(),
        gateway: Some(d.gateway_url()),
        dir: dir.clone(),
    };
    let hangar = Arc::new(Sharing {
        host: host.clone(),
        leads,
        others,
    });
    tokio::spawn(async move {
        if let Err(error) = hangar_discord::run(hangar, options).await {
            panic!("the connector stopped: {error}");
        }
    });
    Side { host, shells, dir }
}

fn channel_of(dir: &std::path::Path, ws: &str) -> Option<String> {
    std::fs::read_to_string(dir.join("discord-layout.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|layout| {
            layout["workspaces"][ws]["channel"]
                .as_str()
                .map(str::to_string)
        })
}

/// A desktop and the node it drives share one bot on one server: each
/// makes and answers in its own channels, the node's categories named
/// after it, and the desktop, which leads, answers where neither made a
/// channel.
#[tokio::test(flavor = "multi_thread")]
async fn a_desktop_and_its_node_share_one_bot() {
    let root = tempfile::tempdir().unwrap();
    let d = Discord::start().await;
    let paired = hangar_discord::Settings {
        token: "secret".into(),
        guild: "g1".into(),
        owner: Some("u1".into()),
        ..hangar_discord::Settings::default()
    };
    let node_channels = Arc::new(Mutex::new(HashSet::new()));
    let desktop = side(
        root.path(),
        "desktop",
        ("shop", "o-desk"),
        paired.clone(),
        true,
        node_channels.clone(),
        &d,
    );
    let node = side(
        root.path(),
        "node",
        ("lab", "o-node"),
        hangar_discord::Settings {
            shared: true,
            label: Some("vps".into()),
            ..paired
        },
        false,
        Arc::default(),
        &d,
    );
    // Both bots hear the gateway before anything is said on it.
    eventually("both bots connected", || {
        (d.identifies() >= 2).then_some(())
    })
    .await;
    let shop = eventually("the desktop's channel", || channel_of(&desktop.dir, "shop")).await;
    let lab = eventually("the node's channel", || channel_of(&node.dir, "lab")).await;
    // What the desktop learns from the node it drives.
    locked(&node_channels).insert(lab.clone());
    let categories: Vec<String> = d
        .calls_to("POST", "/guilds/g1/channels")
        .into_iter()
        .filter(|c| c.body["type"] == 4)
        .map(|c| c.body["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(categories.contains(&"Hangar".to_string()), "{categories:?}");
    assert!(
        categories.contains(&"Hangar · vps".to_string()),
        "{categories:?}"
    );

    // Written in the node's channel: the node's orchestrator alone hears it.
    d.send(
        "MESSAGE_CREATE",
        json!({ "id": "n1", "channel_id": lab,
        "content": "Lance les tests", "author": { "id": "u1" } }),
    );
    eventually("typed on the node", || {
        node.shells
            .typed_into("o-node")
            .contains("Lance les tests")
            .then_some(())
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !desktop
            .shells
            .typed_into("o-desk")
            .contains("Lance les tests")
    );

    // A command is answered once: by the node in its channel, by the
    // desktop in its own, and by the desktop where neither made one.
    let status = json!({ "name": "status" });
    for (id, channel) in [
        ("c1", lab.as_str()),
        ("c2", shop.as_str()),
        ("c3", "general"),
    ] {
        d.send(
            "INTERACTION_CREATE",
            interaction(id, "u1", channel, 2, status.clone(), None),
        );
        eventually("answered", || {
            d.calls_to("POST", &format!("/interactions/{id}/")).pop()
        })
        .await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    for id in ["c1", "c2", "c3"] {
        assert_eq!(
            d.calls_to("POST", &format!("/interactions/{id}/")).len(),
            1,
            "{id} answered once"
        );
    }
    let said = |id: &str| {
        d.calls_to(
            "PATCH",
            &format!("/webhooks/app1/{id}tok/messages/@original"),
        )
        .pop()
        .map(|c| c.body["content"].as_str().unwrap_or_default().to_string())
    };
    let lab_status = eventually("the node's status", || said("c1")).await;
    assert!(lab_status.contains("lab"), "{lab_status}");
    let shop_status = eventually("the desktop's status", || said("c2")).await;
    assert!(shop_status.contains("shop"), "{shop_status}");
    let _ = (&desktop.host, &node.host);
}
