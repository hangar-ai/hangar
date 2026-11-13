//! What the person asks of a workspace its node runs itself that only the
//! node's engines can do (docs/native/nodes.md): land a delivery, stop its
//! workers. This computer's engines know nothing of a mirror, and must not
//! act on one: a delivery landed from here would be run by an engine that
//! does not hold the workspace, beside the node's own. So what is asked
//! goes to the node (`POST /node/own/desk`), whose desk does it as it does
//! for the phone and the Discord connector, and the answer is shown here
//! as a local one's.

use iced::Task;
use serde_json::{Value, json};

use hangar_i18n::{Key, Translator, args};

use super::model::NodeClient;
use super::{client, label, wire};
use crate::message::Message;
use crate::state::Model;
use crate::state::types::{AppState, Workspace};

/// `op` asked of the node `client` reaches, with `args`: what it answers,
/// or why it did not do it, in its words.
pub async fn ask(client: &NodeClient, op: &str, args: Value) -> Result<Value, String> {
    let body = json!({ "op": op, "args": args });
    wire::request(client, "POST", "/node/own/desk", Some(&body))
        .await?
        .json()
}

/// The delivery of `task` landed by its node. The node answers within a
/// few seconds whether it landed yet (`remote_land` waits that long at
/// most), so it is asked until it says how it ended.
pub async fn land(client: &NodeClient, task: &str) -> Result<(), String> {
    loop {
        let answer = ask(client, "remote_land", json!({ "task": task })).await?;
        if answer.get("landed").and_then(Value::as_bool) == Some(true) {
            return Ok(());
        }
    }
}

/// How many workers of workspace `ws` its node stopped.
pub async fn stop_workers(client: &NodeClient, ws: &str) -> Result<u64, String> {
    let answer = ask(client, "stop_workers", json!({ "workspace": ws })).await?;
    Ok(answer.get("stopped").and_then(Value::as_u64).unwrap_or(0))
}

/// The node that is to answer `op`, a call of the phone about a delivery
/// (`remote_delivery`, `remote_land`…): the one that runs the workspace
/// whose worker holds the task named, when that workspace is a node's own.
/// The phone reaches this computer, whose desk reads every workspace it
/// shows; a delivery of a mirror answered here would be landed by this
/// computer's engine. `None` for any other call, and a task of a workspace
/// this computer runs.
pub fn node_for(op: &str, args: &Value, state: &AppState) -> Option<String> {
    if !hangar_engine::mobile::delivery::OPS.contains(&op) {
        return None;
    }
    let task = args.get("task").and_then(Value::as_str).map(str::trim)?;
    // The first worker that holds it, as the desk itself looks for it.
    let holder = state.workspaces.iter().find(|ws| {
        crate::state::roles::workers_of(ws)
            .into_iter()
            .any(|pane| pane.task_id.as_deref() == Some(task))
    })?;
    holder.node.clone().filter(|_| holder.is_hosted())
}

/// A call of the phone handed to node `node` ([`node_for`]), and what it
/// answered. The words are the desk's own language: the phone shows them.
pub async fn forward(node: &str, op: &str, args: Value) -> Result<Value, String> {
    match super::remote::directory(node) {
        Some(client) => ask(&client, op, args).await,
        None => Err(
            "The machine that runs this workspace is not connected: nothing was done.".to_string(),
        ),
    }
}

/// What is said in place of an answer when the node of `ws` cannot be
/// asked.
fn away(m: &Model, node: &str) -> String {
    let node = label(m, node);
    Translator::new(m.locale())
        .t_with(Key::NodesPaneAway, &args!("node" => node.as_str()))
        .into_owned()
}

/// « Merge » on a delivery of `ws`, a workspace its node runs: landed
/// there, and said here as a landing of this computer's is
/// (`control::landed_words`).
pub fn land_task(m: &Model, ws: &Workspace, task: &str) -> Task<Message> {
    let node = ws.node.clone().unwrap_or_default();
    let title = m
        .live
        .board
        .tasks(&crate::board::cache::board_key(ws))
        .and_then(|tasks| tasks.iter().find(|entry| entry.id == task))
        .map_or_else(|| task.to_owned(), |entry| entry.title.clone());
    let locale = m.locale();
    let (workspace, task) = (ws.clone(), task.to_owned());
    let asked = client(m, &node);
    let offline = away(m, &node);
    Task::future(async move {
        let landed = match asked {
            Some(client) => land(&client, &task).await,
            None => Err(offline),
        };
        crate::control::landed_words(&workspace, &task, &title, landed, locale)
    })
    .then(|said| Task::batch(said.into_iter().map(Task::done)))
}

/// « Stop the workers » of `ws`, a workspace its node runs: stopped by the
/// node, and said as for a workspace of this computer.
pub fn stop_all(m: &Model, ws: &Workspace) -> Task<Message> {
    let node = ws.node.clone().unwrap_or_default();
    let locale = m.locale();
    let (id, name) = (ws.id.clone(), ws.name.clone());
    let asked = client(m, &node);
    let offline = away(m, &node);
    Task::perform(
        async move {
            match asked {
                Some(client) => stop_workers(&client, &id).await,
                None => Err(offline),
            }
        },
        move |stopped| crate::orchestration::stopped_words(locale, &name, stopped),
    )
}
