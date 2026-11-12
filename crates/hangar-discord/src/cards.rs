//! The messages the bot sends, as Components V2 (discord-connector.md, "The
//! cards"): each a message body (`flags` and `components`) or a modal,
//! written as JSON.
//!
//! The ids of what the person clicks name what it acts on (ids.rs).

use serde_json::{Value, json};

use hangar_core::chat::Question;

use crate::ids::Click;
use crate::words::Words;

/// `IS_COMPONENTS_V2`: the message is its components alone.
pub const COMPONENTS_V2: u64 = 1 << 15;
/// `SUPPRESS_NOTIFICATIONS`: no push for work in progress.
pub const QUIET: u64 = 1 << 12;
/// `EPHEMERAL`: seen by the one who clicked alone.
pub const EPHEMERAL: u64 = 1 << 6;

/// Hangar's state colours, as the accent bar of a card.
pub mod accent {
    pub const WORKING: u32 = 0x5fd27f;
    pub const WAITING: u32 = 0xe7c06b;
    pub const ACCENT: u32 = 0x3e8bff;
    pub const FAILED: u32 = 0xe5484d;
    pub const DONE: u32 = 0x6b7280;
}

/// Discord's limit of text in one message, kept under (the docs do not
/// state it; libraries cite about 4000).
pub const TEXT_LIMIT: usize = 3_800;

mod style {
    pub const PRIMARY: u8 = 1;
    pub const SECONDARY: u8 = 2;
    pub const SUCCESS: u8 = 3;
    pub const DANGER: u8 = 4;
    pub const LINK: u8 = 5;
}

pub fn text(content: impl Into<String>) -> Value {
    json!({ "type": 10, "content": content.into() })
}

pub fn separator() -> Value {
    json!({ "type": 14, "divider": true, "spacing": 1 })
}

pub fn container(accent: u32, components: Vec<Value>) -> Value {
    json!({ "type": 17, "accent_color": accent, "components": components })
}

pub fn row(components: Vec<Value>) -> Value {
    json!({ "type": 1, "components": components })
}

pub fn button(style: u8, label: &str, click: &Click) -> Value {
    json!({ "type": 2, "style": style, "label": clip(label, 80), "custom_id": click.id() })
}

pub fn link(label: &str, url: &str) -> Value {
    json!({ "type": 2, "style": style::LINK, "label": clip(label, 80), "url": url })
}

/// Up to three texts with `accessory` (a button) beside them.
pub fn section(texts: Vec<String>, accessory: Value) -> Value {
    json!({
        "type": 9,
        "components": texts.into_iter().take(3).map(text).collect::<Vec<_>>(),
        "accessory": accessory,
    })
}

/// A message body of `components`.
pub fn message(components: Vec<Value>, quiet: bool) -> Value {
    let flags = COMPONENTS_V2 | if quiet { QUIET } else { 0 };
    json!({ "flags": flags, "components": components, "allowed_mentions": { "parse": [] } })
}

/// The same, mentioning `owner` (and nobody else): what waits on them.
pub fn calling(components: Vec<Value>, owner: Option<&str>) -> Value {
    let mut body = message(components, false);
    if let Some(owner) = owner {
        body["allowed_mentions"] = json!({ "parse": [], "users": [owner] });
    }
    body
}

/// `<@owner> ` when there is one.
fn mention(owner: Option<&str>) -> String {
    owner.map(|id| format!("<@{id}> ")).unwrap_or_default()
}

/// At most `max` characters, an ellipsis in place of the rest.
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Text that cannot close a code block it is put in.
fn fenced(text: &str) -> String {
    text.replace("```", "`\u{200b}``")
}

/// Claude's answer in pieces Discord takes, cut at paragraph boundaries,
/// a code block closed and opened again across a cut.
pub fn split_answer(answer: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut in_code: Option<String> = None;
    for line in answer.lines() {
        let fence = line.trim_start().starts_with("```");
        let would = current.len() + line.len() + 1;
        if would > TEXT_LIMIT && !current.is_empty() {
            if let Some(open) = &in_code {
                current.push_str("```");
                pieces.push(std::mem::take(&mut current));
                current.push_str(open);
                current.push('\n');
            } else {
                pieces.push(std::mem::take(&mut current));
            }
        }
        // A single line longer than a piece: cut it as it is.
        let mut rest = line;
        while rest.len() > TEXT_LIMIT {
            let mut cut = TEXT_LIMIT;
            while !rest.is_char_boundary(cut) {
                cut -= 1;
            }
            current.push_str(&rest[..cut]);
            pieces.push(std::mem::take(&mut current));
            rest = &rest[cut..];
        }
        current.push_str(rest);
        current.push('\n');
        if fence {
            in_code = match in_code {
                Some(_) => None,
                None => Some(line.trim().to_string()),
            };
        }
    }
    if !current.trim().is_empty() {
        pieces.push(current.trim_end().to_string());
    }
    pieces
        .into_iter()
        .map(|piece| piece.trim_end().to_string())
        .filter(|piece| !piece.is_empty())
        .collect()
}

/// Claude's answer: one message per piece.
pub fn answer(answer: &str) -> Vec<Value> {
    split_answer(answer)
        .into_iter()
        .map(|piece| message(vec![text(piece)], false))
        .collect()
}

/// A turn under way: who, since when, its last calls, and Stop.
pub fn turn(words: Words, name: &str, since_secs: u64, calls: &[String], pane: &str) -> Value {
    let mut parts = vec![text(format!(
        "**{}** {} · <t:{since_secs}:R>",
        clip(name, 60),
        words.working()
    ))];
    if !calls.is_empty() {
        let lines: Vec<String> = calls
            .iter()
            .rev()
            .take(8)
            .rev()
            .map(|call| format!("-# ⏵ {}", clip(call, 150)))
            .collect();
        parts.push(text(lines.join("\n")));
    }
    parts.push(row(vec![button(
        style::SECONDARY,
        words.stop(),
        &Click::Stop {
            pane: pane.to_string(),
        },
    )]));
    message(vec![container(accent::WORKING, parts)], true)
}

/// A turn over, folded to one line.
pub fn turn_folded(words: Words, name: &str, calls: usize, minutes: u64) -> Value {
    message(
        vec![text(format!(
            "-# **{}** · {}",
            clip(name, 60),
            words.actions(calls, minutes)
        ))],
        true,
    )
}

/// A line of Hangar's news.
pub fn line(content: &str) -> Value {
    message(vec![text(format!("-# {}", clip(content, 1_900)))], true)
}

/// A permission the agent waits on: what it would run, Allow and Deny.
pub fn permission(
    words: Words,
    name: &str,
    title: &str,
    detail: Option<&str>,
    pane: &str,
    owner: Option<&str>,
) -> Value {
    let mut parts = vec![text(format!(
        "{}**{}** {} · {}",
        mention(owner),
        clip(name, 60),
        words.asks_permission(),
        clip(title, 200)
    ))];
    if let Some(detail) = detail.filter(|d| !d.trim().is_empty()) {
        parts.push(text(format!("```\n{}\n```", fenced(&clip(detail, 3_000)))));
    }
    let click = |allow| Click::Permit {
        pane: pane.to_string(),
        allow,
    };
    parts.push(row(vec![
        button(style::SUCCESS, words.allow(), &click(true)),
        button(style::DANGER, words.deny(), &click(false)),
    ]));
    calling(vec![container(accent::WAITING, parts)], owner)
}

/// Questions the agent waits on. One question picked from: a section per
/// option with Choose beside it, and "Other answer…"; several (or one with
/// several answers): their text, and one button opening the modal.
pub fn questions(
    words: Words,
    name: &str,
    asked: &[Question],
    pane: &str,
    owner: Option<&str>,
) -> Value {
    let mut parts = vec![text(format!(
        "{}**{}** {} :",
        mention(owner),
        clip(name, 60),
        words.asks()
    ))];
    let single = asked.len() == 1 && !asked[0].multi_select;
    if single {
        let question = &asked[0];
        parts.push(text(format!("### {}", clip(&question.question, 300))));
        for (n, option) in question.options.iter().enumerate().take(10) {
            let mut lines = vec![format!("**{}**", clip(&option.label, 200))];
            if !option.description.trim().is_empty() {
                lines.push(format!("-# {}", clip(&option.description, 300)));
            }
            parts.push(section(
                lines,
                button(
                    style::SECONDARY,
                    words.choose(),
                    &Click::Choose {
                        pane: pane.to_string(),
                        option: n,
                    },
                ),
            ));
        }
        parts.push(row(vec![button(
            style::SECONDARY,
            words.other_answer(),
            &Click::Other {
                pane: pane.to_string(),
            },
        )]));
    } else {
        for question in asked {
            let options: Vec<String> = question
                .options
                .iter()
                .map(|option| format!("`{}`", clip(&option.label, 60)))
                .collect();
            parts.push(text(format!(
                "**{}** — {}\n-# {}",
                clip(&question.header, 40),
                clip(&question.question, 300),
                options.join(" · ")
            )));
        }
        parts.push(row(vec![button(
            style::PRIMARY,
            &words.answer_questions(asked.len()),
            &Click::AnswerAll {
                pane: pane.to_string(),
            },
        )]));
    }
    calling(vec![container(accent::WAITING, parts)], owner)
}

/// The modal of several questions: a select per question (four at most,
/// as AskUserQuestion asks), and a field for the person's own words.
pub fn questions_modal(words: Words, asked: &[Question], pane: &str) -> Value {
    let mut components: Vec<Value> = asked
        .iter()
        .take(4)
        .enumerate()
        .map(|(n, question)| {
            let options: Vec<Value> = question
                .options
                .iter()
                .take(25)
                .enumerate()
                .map(|(i, option)| {
                    let mut entry =
                        json!({ "label": clip(&option.label, 100), "value": i.to_string() });
                    if !option.description.trim().is_empty() {
                        entry["description"] = json!(clip(&option.description, 100));
                    }
                    entry
                })
                .collect();
            json!({
                "type": 18,
                "label": clip(&question.header, 45),
                "description": clip(&question.question, 100),
                "component": {
                    "type": 3,
                    "custom_id": format!("q{n}"),
                    "options": options,
                    "min_values": 0,
                    "max_values": if question.multi_select { options.len().max(1) } else { 1 },
                    "required": false,
                },
            })
        })
        .collect();
    components.push(other_field(words));
    json!({
        "custom_id": Click::AnswerAll { pane: pane.to_string() }.id(),
        "title": clip(words.your_answer(), 45),
        "components": components,
    })
}

/// The modal of one question's own answer.
pub fn other_modal(words: Words, question: &Question, pane: &str) -> Value {
    let mut field = other_field(words);
    field["description"] = json!(clip(&question.question, 100));
    json!({
        "custom_id": Click::Other { pane: pane.to_string() }.id(),
        "title": clip(words.your_answer(), 45),
        "components": [field],
    })
}

fn other_field(words: Words) -> Value {
    json!({
        "type": 18,
        "label": clip(words.other_or_details(), 45),
        "component": { "type": 4, "custom_id": "other", "style": 2, "required": false, "max_length": 2000 },
    })
}

/// A plan to approve: its text (an excerpt and the whole as `plan.md` when
/// long), Approve and Keep planning….
pub fn plan(
    words: Words,
    name: &str,
    plan: &str,
    pane: &str,
    owner: Option<&str>,
) -> (Value, Option<String>) {
    let long = plan.len() > 3_000;
    let shown = if long {
        clip(plan, 2_500)
    } else {
        plan.to_string()
    };
    let mut parts = vec![
        text(format!(
            "{}**{}** {}",
            mention(owner),
            clip(name, 60),
            words.proposes_plan()
        )),
        separator(),
        text(shown),
    ];
    if long {
        parts.push(text(format!("-# {}", words.whole_plan())));
        parts.push(json!({ "type": 13, "file": { "url": "attachment://plan.md" } }));
    }
    let click = |approve| Click::Plan {
        pane: pane.to_string(),
        approve,
    };
    parts.push(row(vec![
        button(style::SUCCESS, words.approve_plan(), &click(true)),
        button(style::SECONDARY, words.keep_planning(), &click(false)),
    ]));
    (
        calling(vec![container(accent::WAITING, parts)], owner),
        long.then(|| plan.to_string()),
    )
}

/// What to change in the plan.
pub fn plan_modal(words: Words, pane: &str) -> Value {
    json!({
        "custom_id": Click::Plan { pane: pane.to_string(), approve: false }.id(),
        "title": clip(words.keep_planning(), 45),
        "components": [{
            "type": 18,
            "label": clip(words.what_to_change(), 45),
            "component": { "type": 4, "custom_id": "text", "style": 2, "required": true, "max_length": 4000 },
        }],
    })
}

/// A card once answered: what was done, by whom, when; no button left.
pub fn settled(said: &str, by: Option<&str>, at_secs: u64, words: Words) -> Value {
    let who = by
        .map(|id| format!(" {} <@{id}>", words.by()))
        .unwrap_or_default();
    message(
        vec![container(
            accent::DONE,
            vec![text(format!("✅ {said}{who} · <t:{at_secs}:R>"))],
        )],
        true,
    )
}

/// A worker that started: its line, and the button that opens its thread,
/// or the link to it once open.
pub fn worker_started(
    words: Words,
    worker: &str,
    task: &str,
    pane: &str,
    thread: Option<&str>,
) -> Value {
    let accessory = match thread {
        Some(url) => link(words.see_thread(), url),
        None => button(
            style::SECONDARY,
            words.open_thread(),
            &Click::Thread {
                pane: pane.to_string(),
            },
        ),
    };
    message(
        vec![section(
            vec![format!(
                "🚀 **{}** {} « {} »",
                clip(worker, 60),
                words.started(),
                clip(task, 200)
            )],
            accessory,
        )],
        true,
    )
}

/// What a delivery brings, to land or send back.
pub struct Delivery<'a> {
    pub ws: &'a str,
    pub task: &'a str,
    pub title: &'a str,
    pub worker: &'a str,
    pub branch: &'a str,
    pub target: &'a str,
    /// path, additions, deletions.
    pub files: Vec<(String, u32, u32)>,
    pub more_files: bool,
    pub summary: Option<&'a str>,
    pub check: Option<String>,
    pub review: Option<String>,
}

pub fn delivery(words: Words, d: &Delivery<'_>, owner: Option<&str>) -> Value {
    // The branches are not known when the delivery could not be read: the
    // card still calls the person, with what the journal said of it.
    let branches = if d.branch.is_empty() {
        String::new()
    } else {
        format!(" · `{}` → `{}`", clip(d.branch, 80), clip(d.target, 80))
    };
    let mut parts = vec![text(format!(
        "{}**{}** · {}\n-# {}{branches}",
        mention(owner),
        words.delivery_ready(),
        clip(d.title, 200),
        clip(d.worker, 60),
    ))];
    if let Some(summary) = d.summary.filter(|s| !s.trim().is_empty()) {
        parts.push(text(clip(summary, 1_200)));
    }
    if !d.files.is_empty() {
        let mut lines: Vec<String> = d
            .files
            .iter()
            .take(20)
            .map(|(path, add, del)| format!("+{add:<4} -{del:<4} {}", fenced(&clip(path, 90))))
            .collect();
        if d.more_files || d.files.len() > 20 {
            lines.push(format!("… {}", words.more_files()));
        }
        parts.push(text(format!("```diff\n{}\n```", lines.join("\n"))));
    }
    let mut verdicts = Vec::new();
    if let Some(check) = &d.check {
        verdicts.push(format!("**{}** {}", words.check(), clip(check, 300)));
    }
    if let Some(review) = &d.review {
        verdicts.push(format!("**{}** {}", words.review(), clip(review, 600)));
    }
    if !verdicts.is_empty() {
        parts.push(text(verdicts.join("\n")));
    }
    let click = |land| Click::Deliver {
        ws: d.ws.to_string(),
        task: d.task.to_string(),
        land,
    };
    parts.push(row(vec![
        button(style::SUCCESS, words.land(), &click(true)),
        button(style::SECONDARY, words.send_back(), &click(false)),
    ]));
    calling(vec![container(accent::ACCENT, parts)], owner)
}

/// What to fix in a delivery sent back.
pub fn send_back_modal(words: Words, ws: &str, task: &str) -> Value {
    json!({
        "custom_id": Click::Deliver { ws: ws.to_string(), task: task.to_string(), land: false }.id(),
        "title": clip(words.send_back(), 45),
        "components": [{
            "type": 18,
            "label": clip(words.what_to_fix(), 45),
            "component": { "type": 4, "custom_id": "text", "style": 2, "required": true, "max_length": 4000 },
        }],
    })
}

/// One row of the dashboard.
pub struct Row<'a> {
    pub name: &'a str,
    pub state: &'a str,
    pub since_secs: Option<u64>,
    /// A worker's: its pane, and its thread's link when open.
    pub worker: Option<(&'a str, Option<&'a str>)>,
}

/// The pinned card of a workspace: each agent, what it does and since
/// when, a worker's thread to open or see.
pub fn dashboard(words: Words, ws_name: &str, rows: &[Row<'_>], board: &str) -> Value {
    let mut parts = vec![text(format!(
        "## {} · {}",
        clip(ws_name, 80),
        words.dashboard()
    ))];
    // Discord takes forty components a message, nested ones counted: the
    // container and the title, the board's line and its separator, one a
    // plain row, three a worker's (its section, its text, its button). The
    // rows past that share one text, without their buttons.
    let mut used = 2 + if board.is_empty() { 0 } else { 2 };
    let mut rest: Vec<String> = Vec::new();
    for row in rows {
        let since = row
            .since_secs
            .map(|s| format!(" · <t:{s}:R>"))
            .unwrap_or_default();
        let line = format!("**{}** — {}{since}", clip(row.name, 60), row.state);
        let cost = if row.worker.is_some() { 3 } else { 1 };
        if !rest.is_empty() || used + cost + 1 > COMPONENTS_MAX {
            rest.push(line);
            continue;
        }
        used += cost;
        match row.worker {
            Some((pane, thread)) => {
                let accessory = match thread {
                    Some(url) => link(words.see_thread(), url),
                    None => button(
                        style::SECONDARY,
                        words.open_thread(),
                        &Click::Thread {
                            pane: pane.to_string(),
                        },
                    ),
                };
                parts.push(section(vec![line], accessory));
            }
            None => parts.push(text(line)),
        }
    }
    if !rest.is_empty() {
        parts.push(text(clip(&rest.join("\n"), 2_000)));
    }
    if !board.is_empty() {
        parts.push(separator());
        parts.push(text(format!("-# {board}")));
    }
    message(vec![container(accent::ACCENT, parts)], true)
}

/// The components Discord takes in one message, nested ones counted.
pub const COMPONENTS_MAX: usize = 40;

#[cfg(test)]
mod tests {
    use super::*;
    use hangar_core::chat::QuestionOption;
    use hangar_i18n::Locale;

    fn fr() -> Words {
        Words::of(Some(Locale::Fr))
    }

    fn components(value: &Value) -> Vec<Value> {
        let mut out = Vec::new();
        fn walk(value: &Value, out: &mut Vec<Value>) {
            if value.get("type").is_some() {
                out.push(value.clone());
            }
            for key in ["components", "accessory", "component"] {
                match value.get(key) {
                    Some(Value::Array(items)) => items.iter().for_each(|i| walk(i, out)),
                    Some(item @ Value::Object(_)) => walk(item, out),
                    _ => {}
                }
            }
        }
        if let Some(Value::Array(items)) = value.get("components") {
            items.iter().for_each(|i| walk(i, &mut out));
        }
        out
    }

    fn question(text: &str, options: &[&str], multi: bool) -> Question {
        Question {
            question: text.into(),
            header: "Choix".into(),
            multi_select: multi,
            options: options
                .iter()
                .map(|label| QuestionOption {
                    label: (*label).into(),
                    description: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn every_card_is_components_v2_and_within_discord_s_count() {
        let w = fr();
        let cards = [
            turn(w, "Leo", 1, &["Read src/a.rs".into()], "p1"),
            permission(w, "Leo", "Bash", Some("rm -rf build"), "p1", Some("42")),
            questions(
                w,
                "Leo",
                &[question("Quelle base ?", &["Postgres", "SQLite"], false)],
                "p1",
                None,
            ),
            plan(w, "Leo", "# Plan\n- un", "p1", None).0,
            worker_started(w, "Ada", "search-index", "p2", None),
            dashboard(
                w,
                "Shop",
                &[Row {
                    name: "Ada",
                    state: "travaille",
                    since_secs: Some(1),
                    worker: Some(("p2", None)),
                }],
                "3 tâches",
            ),
        ];
        for card in cards {
            assert_eq!(
                card["flags"].as_u64().unwrap() & COMPONENTS_V2,
                COMPONENTS_V2
            );
            assert!(card.get("content").is_none());
            assert!(components(&card).len() <= 40, "{card}");
            for component in components(&card) {
                if let Some(id) = component.get("custom_id").and_then(Value::as_str) {
                    assert!(id.len() <= 100, "{id}");
                }
            }
        }
    }

    #[test]
    fn a_dashboard_of_sixteen_agents_stays_within_what_discord_takes() {
        // A workspace holds sixteen panes: an orchestrator and fifteen
        // workers, three components a worker.
        let names: Vec<String> = (0..15).map(|n| format!("Worker {n}")).collect();
        let panes: Vec<String> = (0..15).map(|n| format!("p{n}")).collect();
        let mut rows = vec![Row {
            name: "Leo",
            state: "travaille",
            since_secs: Some(1),
            worker: None,
        }];
        for (name, pane) in names.iter().zip(&panes) {
            rows.push(Row {
                name,
                state: "travaille",
                since_secs: Some(1),
                worker: Some((pane, None)),
            });
        }
        let card = dashboard(fr(), "Shop", &rows, "3 tâches");
        assert!(components(&card).len() <= COMPONENTS_MAX, "{card}");
        // Every agent is still named, the last ones without their button.
        let flat = card.to_string();
        for name in &names {
            assert!(flat.contains(name.as_str()), "{name}");
        }
        assert!(flat.contains("t:p0") && !flat.contains("t:p14"));
    }

    #[test]
    fn a_permission_mentions_its_owner_alone_and_offers_allow_and_deny() {
        let card = permission(fr(), "Leo", "Bash", Some("npm test"), "p1", Some("42"));
        assert_eq!(card["allowed_mentions"]["users"], json!(["42"]));
        let flat = card.to_string();
        assert!(flat.contains("<@42>") && flat.contains("npm test"));
        let ids: Vec<String> = components(&card)
            .iter()
            .filter_map(|c| {
                c.get("custom_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect();
        assert_eq!(ids, ["p:p1:y", "p:p1:n"]);
    }

    #[test]
    fn one_question_is_picked_from_and_several_open_a_modal() {
        let one = questions(
            fr(),
            "Leo",
            &[question("Base ?", &["A", "B"], false)],
            "p1",
            None,
        );
        let ids: Vec<String> = components(&one)
            .iter()
            .filter_map(|c| {
                c.get("custom_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect();
        assert_eq!(ids, ["c:p1:0", "c:p1:1", "o:p1"]);
        let asked = [
            question("Base ?", &["A", "B"], false),
            question("Tests ?", &["Oui", "Non"], true),
        ];
        let several = questions(fr(), "Leo", &asked, "p1", None);
        assert!(several.to_string().contains("Répondre aux 2 questions"));
        let modal = questions_modal(fr(), &asked, "p1");
        assert_eq!(modal["custom_id"], "a:p1");
        assert_eq!(modal["components"].as_array().unwrap().len(), 3);
        assert_eq!(modal["components"][1]["component"]["max_values"], 2);
    }

    #[test]
    fn a_long_plan_is_an_excerpt_and_a_file() {
        let long = "x".repeat(5_000);
        let (card, file) = plan(fr(), "Leo", &long, "p1", None);
        assert_eq!(file.as_deref(), Some(long.as_str()));
        assert!(card.to_string().contains("attachment://plan.md"));
        let (_, none) = plan(fr(), "Leo", "short", "p1", None);
        assert_eq!(none, None);
    }

    #[test]
    fn a_long_answer_is_cut_between_paragraphs_with_its_code_kept_whole() {
        let mut answer = String::new();
        for n in 0..300 {
            answer.push_str(&format!("Paragraph {n} of the answer.\n"));
        }
        answer.push_str("```rust\n");
        for n in 0..200 {
            answer.push_str(&format!("let x{n} = {n};\n"));
        }
        answer.push_str("```\nDone.");
        let pieces = split_answer(&answer);
        assert!(pieces.len() >= 3, "{}", pieces.len());
        for piece in &pieces {
            assert!(piece.len() <= TEXT_LIMIT + 16, "{}", piece.len());
            assert_eq!(piece.matches("```").count() % 2, 0, "{piece}");
        }
        assert!(pieces.last().unwrap().ends_with("Done."));
    }

    #[test]
    fn a_worker_s_line_opens_its_thread_or_links_to_it() {
        let closed = worker_started(fr(), "Ada", "search-index", "p2", None);
        assert!(closed.to_string().contains("\"custom_id\":\"t:p2\""));
        assert!(closed.to_string().contains("Ouvrir le fil"));
        let open = worker_started(
            fr(),
            "Ada",
            "search-index",
            "p2",
            Some("https://discord.com/channels/1/2"),
        );
        assert!(open.to_string().contains("Voir le fil"));
        assert!(open.to_string().contains("\"style\":5"));
    }
}
