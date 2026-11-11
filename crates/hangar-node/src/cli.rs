//! The node's own workspaces from a shell on the server (own.rs): asked of
//! the running daemon over its API, with the token in `node.json`.
//!
//! - `hangar-node workspaces`: each with its panes and what they are doing;
//! - `hangar-node workspace add <folder> [--name <name>] [--no-orchestrator]`;
//! - `hangar-node workspace remove <id>`;
//! - `hangar-node send <pane> <text…>`: queued for the pane's session.
#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::time::Duration;

use serde_json::{json, Value};

use crate::state;

/// One request to the running daemon: its status and JSON answer.
pub(crate) fn call(method: &str, path: &str, body: Option<Value>) -> Result<(u16, Value), String> {
    let dir = state::dir()?;
    let node = state::running(&dir)
        .ok_or("hangar-node is not running here: start it with `hangar-node serve --detach`")?;
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", node.port))
        .map_err(|err| format!("the node does not answer: {err}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|err| err.to_string())?;
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    write!(
        socket,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        node.token,
        body.len()
    )
    .map_err(|err| err.to_string())?;
    let mut raw = Vec::new();
    socket
        .read_to_end(&mut raw)
        .map_err(|err| err.to_string())?;
    let raw = String::from_utf8_lossy(&raw);
    let (head, rest) = raw
        .split_once("\r\n\r\n")
        .ok_or("the node's answer was cut short")?;
    let status = head
        .get(9..12)
        .and_then(|code| code.parse().ok())
        .ok_or("the node's answer has no status")?;
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        dechunk(rest)
    } else {
        rest.to_string()
    };
    Ok((status, serde_json::from_str(&body).unwrap_or(Value::Null)))
}

fn dechunk(mut rest: &str) -> String {
    let mut out = String::new();
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if size == 0 || after.len() < size {
            break;
        }
        out.push_str(&after[..size]);
        rest = after[size..].trim_start_matches("\r\n");
    }
    out
}

/// The answer, or the node's refusal in its own words.
fn answered(outcome: (u16, Value)) -> Result<Value, String> {
    let (status, body) = outcome;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(body["error"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("the node answered {status}")))
    }
}

pub fn workspaces() -> Result<(), String> {
    let listing = answered(call("GET", "/node/own", None)?)?;
    let workspaces = listing["workspaces"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if workspaces.is_empty() {
        println!("No workspace of the node's own yet: hangar-node workspace add <folder>");
        return Ok(());
    }
    for ws in workspaces {
        println!(
            "{}  {}  {}",
            ws["id"].as_str().unwrap_or_default(),
            ws["name"].as_str().unwrap_or_default(),
            ws["cwd"].as_str().unwrap_or_default()
        );
        for pane in ws["panes"].as_array().cloned().unwrap_or_default() {
            let role = pane["role"].as_str().unwrap_or("");
            let activity = pane["activity"].as_str().unwrap_or("—");
            println!(
                "    {}  {}{}  {activity}",
                pane["id"].as_str().unwrap_or_default(),
                pane["name"].as_str().unwrap_or_default(),
                if role.is_empty() {
                    String::new()
                } else {
                    format!(" ({role})")
                }
            );
        }
    }
    Ok(())
}

pub fn add(args: &[&str]) -> Result<(), String> {
    let mut folder = None;
    let mut name = String::new();
    let mut orchestrator = true;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "--name" => name = rest.next().ok_or("--name wants a name")?.to_string(),
            "--no-orchestrator" => orchestrator = false,
            other if folder.is_none() => folder = Some(other.to_string()),
            other => return Err(format!("unexpected: {other}")),
        }
    }
    let folder = folder.ok_or("workspace add wants a folder")?;
    let folder = std::fs::canonicalize(&folder)
        .map_err(|err| format!("{folder}: {err}"))?
        .to_string_lossy()
        .into_owned();
    let made = answered(call(
        "POST",
        "/node/own/workspaces",
        Some(json!({ "name": name, "cwd": folder, "orchestrator": orchestrator })),
    )?)?;
    println!("{}", made["id"].as_str().unwrap_or_default());
    Ok(())
}

pub fn remove(id: &str) -> Result<(), String> {
    answered(call("DELETE", &format!("/node/own/workspaces/{id}"), None)?)?;
    Ok(())
}

pub fn send(pane: &str, words: &[&str]) -> Result<(), String> {
    let text = words.join(" ");
    answered(call(
        "POST",
        &format!("/node/own/panes/{pane}/send"),
        Some(json!({ "text": text })),
    )?)?;
    Ok(())
}
