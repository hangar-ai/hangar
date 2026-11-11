//! Answering through the ask a pane's hangar-bridge mod holds open, rather
//! than through keys typed into Claude Code's dialog.
//!
//! When Claude Code is about to ask the person something, the mod reports the
//! ask with an id and waits in core for its answer (core's bridge.rs, "Asks
//! the mod holds open"), while the dialog stays up in the terminal. An answer
//! given here reaches Claude Code as the decision the person made — the
//! labels they picked, their own words, a yes or a no — with no screen read
//! back and no key that could land elsewhere.
//!
//! Keys stay the way for what no mod holds: a pane on a node (its asks are
//! held there), a Claude Code that runs without the mod, an ask reported by
//! an older mod. And the mod must take the answer within `TAKE_MS`: one that
//! is not there to take it (its hook skipped, Claude Code gone) has it taken
//! back, and the keys are typed after all.

use serde_json::{Map, Value, json};

use hangar_core::bridge::{self, HeldAsk};
use hangar_core::chat::Question;

use crate::control::port::Port;
use crate::nodes::away;
use crate::state::{Pane, Workspace};

use super::answer::Choice;

/// How long the mod gets to take an answer.
const TAKE_MS: u64 = 3_000;
const TAKE_POLL_MS: u64 = 50;

pub const ASK_QUESTION: &str = "AskUserQuestion";

/// The asks the pane's mod holds open here, newest first; none for a pane on
/// a node.
fn held(ws: &Workspace, pane: &Pane) -> Vec<HeldAsk> {
    if away::node_of(ws, pane).is_some() {
        return Vec::new();
    }
    let mut asks = bridge::held_asks(&pane.id);
    asks.reverse();
    asks
}

/// The ask held for the permission the pane shows on `tool`: the one with
/// the input the mod reported for the dialog (`input`), else the newest on
/// that tool.
pub fn permission(
    ws: &Workspace,
    pane: &Pane,
    tool: &str,
    input: Option<&Value>,
) -> Option<HeldAsk> {
    let asks: Vec<HeldAsk> = held(ws, pane)
        .into_iter()
        .filter(|ask| ask.tool == tool && ask.tool != ASK_QUESTION)
        .collect();
    input
        .and_then(|input| asks.iter().find(|ask| ask.input == *input).cloned())
        .or_else(|| asks.into_iter().next())
}

/// The ask held for question `tool_use_id`: the one about that call, else,
/// for an ask that could not name its call, the one asking these questions.
pub fn question(
    ws: &Workspace,
    pane: &Pane,
    tool_use_id: &str,
    questions: &[Question],
) -> Option<HeldAsk> {
    let asks: Vec<HeldAsk> = held(ws, pane)
        .into_iter()
        .filter(|ask| ask.tool == ASK_QUESTION)
        .collect();
    if let Some(ask) = asks
        .iter()
        .find(|ask| ask.tool_use_id.as_deref() == Some(tool_use_id))
    {
        return Some(ask.clone());
    }
    asks.into_iter()
        .find(|ask| ask.tool_use_id.is_none() && asks_the_same(&ask.input, questions))
}

/// The questions an ask's input puts are these, word for word.
fn asks_the_same(input: &Value, questions: &[Question]) -> bool {
    let Some(asked) = input.get("questions").and_then(Value::as_array) else {
        return false;
    };
    asked.len() == questions.len()
        && asked.iter().zip(questions).all(|(asked, question)| {
            asked.get("question").and_then(Value::as_str) == Some(question.question.as_str())
        })
}

/// A yes or a no to a permission. A no stops the turn, as Escape in the
/// dialog does: the agent waits for what the person says next.
pub fn permission_decision(allow: bool) -> Value {
    if allow {
        json!({ "behavior": "allow" })
    } else {
        json!({ "behavior": "deny", "interrupt": true })
    }
}

/// A plan approved, or sent back to keep planning: the model reads why, and
/// what to change when the person said. It is not stopped: it plans again.
pub fn plan_decision(approve: bool, feedback: Option<&str>) -> Value {
    if approve {
        return json!({ "behavior": "allow" });
    }
    let message = match feedback.map(str::trim).filter(|words| !words.is_empty()) {
        Some(words) => format!(
            "The person wants to keep planning before anything is implemented. What to change in the plan: {words}"
        ),
        None => {
            "The person wants to keep planning: revise the plan before anything is implemented."
                .to_string()
        }
    };
    json!({ "behavior": "deny", "message": message })
}

/// The answers as the dialog gives them: each question's text to the labels
/// picked and the person's own words, joined by commas.
pub fn question_decision(questions: &[Question], choices: &[Choice]) -> Value {
    let answers: Map<String, Value> = questions
        .iter()
        .zip(choices)
        .map(|(question, choice)| {
            let said: Vec<String> = choice
                .picked
                .iter()
                .filter_map(|&at| question.options.get(at).map(|option| option.label.clone()))
                .chain(choice.other.clone())
                .collect();
            (question.question.clone(), Value::String(said.join(", ")))
        })
        .collect();
    json!({ "behavior": "allow", "answers": answers })
}

/// Gives ask `ask` of the pane its answer and waits for the mod to take it.
/// False when it could not be given, or nobody took it: it is taken back,
/// and the caller types the keys instead.
pub async fn give(port: &Port, pane: &Pane, ask: &HeldAsk, decision: Value) -> bool {
    if !bridge::answer_ask(&pane.id, &ask.id, decision) {
        return false;
    }
    let mut waited = 0;
    while waited < TAKE_MS {
        // Taken, or the ask ended meanwhile: the dialog is down either way.
        if !bridge::answer_untaken(&pane.id, &ask.id) {
            return true;
        }
        port.sleep(TAKE_POLL_MS).await;
        // The mod's wait runs on this runtime too: let it take the answer.
        tokio::task::yield_now().await;
        waited += TAKE_POLL_MS;
    }
    !bridge::withdraw_answer(&pane.id, &ask.id)
}

#[cfg(test)]
mod tests {
    use hangar_core::chat::QuestionOption;

    use super::*;

    fn question(text: &str, labels: &[&str]) -> Question {
        Question {
            question: text.into(),
            header: String::new(),
            multi_select: true,
            options: labels
                .iter()
                .map(|label| QuestionOption {
                    label: (*label).into(),
                    description: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn answers_are_the_labels_then_the_persons_words() {
        let questions = [
            question("Which color?", &["Red", "Blue"]),
            question("Which sizes?", &["S", "M", "L"]),
        ];
        let choices = [
            Choice {
                picked: vec![1],
                other: None,
            },
            Choice {
                picked: vec![0, 2],
                other: Some("XXL too".into()),
            },
        ];
        assert_eq!(
            question_decision(&questions, &choices),
            json!({
                "behavior": "allow",
                "answers": { "Which color?": "Blue", "Which sizes?": "S, L, XXL too" },
            })
        );
    }

    #[test]
    fn a_no_stops_the_turn_as_escape_does() {
        assert_eq!(permission_decision(true), json!({ "behavior": "allow" }));
        assert_eq!(
            permission_decision(false),
            json!({ "behavior": "deny", "interrupt": true })
        );
    }

    #[test]
    fn a_plan_sent_back_carries_what_to_change_and_does_not_stop_the_turn() {
        assert_eq!(
            plan_decision(true, Some("ignored")),
            json!({ "behavior": "allow" })
        );
        let kept = plan_decision(false, Some("  Split step 2 in two  "));
        assert_eq!(kept["behavior"], "deny");
        assert!(
            kept["message"]
                .as_str()
                .unwrap()
                .ends_with(": Split step 2 in two")
        );
        assert!(kept.get("interrupt").is_none());
        let plain = plan_decision(false, Some("   "));
        assert!(plain["message"].as_str().unwrap().contains("keep planning"));
    }

    #[test]
    fn an_ask_without_its_call_is_matched_by_its_questions() {
        let questions = [question("Which color?", &["Red"])];
        let input = json!({ "questions": [{ "question": "Which color?", "options": [] }] });
        assert!(asks_the_same(&input, &questions));
        let other = json!({ "questions": [{ "question": "Which size?" }] });
        assert!(!asks_the_same(&other, &questions));
        assert!(!asks_the_same(&json!({}), &questions));
    }
}
