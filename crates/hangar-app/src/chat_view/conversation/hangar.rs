//! Hangar's messages to the orchestrator, read for the person: what the
//! hangar-bridge mod labels them with says what they are (`labelled` in its
//! register.js), and the news of the workspace reads back line by line
//! (`orchestration::heard`).
//!
//! - `[Hangar] Event — …`: the news of the workspace, a line per thing that
//!   happened, then what to do next, which is for the model alone;
//! - `[Hangar] From agent "Ada" — …`: a message an agent sent;
//! - `[Hangar] Briefing — …`: the state of the work, handed to a
//!   conversation that starts.

use crate::orchestration::heard::{Heard, read_line};

/// One of Hangar's messages, as the person reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// The news of the workspace, in order.
    News(Vec<Line>),
    /// A message another agent sent the orchestrator.
    Agent { from: String, text: String },
    /// The state of the work, for a conversation that starts.
    Briefing(String),
    /// Anything else Hangar said.
    Other(String),
}

/// A line of news: what Hangar wrote it from, or its words when it reads as
/// none of that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Heard(Heard),
    Said(String),
}

const EVENT: &str = "[Hangar] Event";
const FROM_AGENT: &str = "[Hangar] From agent ";
const BRIEFING: &str = "[Hangar] Briefing";

/// What follows the label's line and the blank line under it.
fn body(text: &str) -> &str {
    text.split_once('\n').map_or("", |(_, rest)| rest).trim()
}

/// The workspace's news, without the lines for the model alone: the count
/// at the top, the tasks ready to start, what to do once all are done, and
/// the pointer to the rest.
fn news(body: &str) -> Vec<Line> {
    body.lines()
        .map(str::trim)
        .filter_map(|line| {
            let item = line.strip_prefix("- ")?;
            if item.starts_with("…and ") {
                return None;
            }
            Some(match read_line(item) {
                Some(heard) => Line::Heard(heard),
                None => Line::Said(item.to_string()),
            })
        })
        .collect()
}

/// A message Hangar put in the conversation, read.
pub fn note(text: &str) -> Note {
    let text = text.trim();
    if text.starts_with(EVENT) {
        let body = body(text);
        let lines = news(body);
        if lines.is_empty() {
            return Note::Other(body.to_string());
        }
        return Note::News(lines);
    }
    if let Some(rest) = text.strip_prefix(FROM_AGENT)
        && let Some(quoted) = rest.strip_prefix('"')
        && let Some((from, _)) = quoted.split_once('"')
    {
        return Note::Agent {
            from: from.to_string(),
            text: body(text).to_string(),
        };
    }
    if text.starts_with(BRIEFING) {
        return Note::Briefing(body(text).to_string());
    }
    Note::Other(text.trim_start_matches("[Hangar]").trim().to_string())
}

/// The first line of `text`, cut to `at_most` characters.
pub fn first_line(text: &str, at_most: usize) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if line.chars().count() <= at_most {
        return line.to_string();
    }
    let cut: String = line.chars().take(at_most).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::events::DeliveryOutcome;

    #[test]
    fn hangar_s_news_reads_as_its_lines_without_what_is_for_the_model() {
        let text = "[Hangar] Event — not typed by the person.\n\n\
            2 updates in workspace \"Shop\":\n\
            - Delivery of \"Search box\" [t1] by Ada landed: merged on the branch, task done\n\
            - Bo started on \"Filters\" [t2]\n\
            - Something new\n\
            - …and 3 more: board_list_tasks and worker_list have the rest.\n\
            Ready to start: \"Filters\" [t2].";
        assert_eq!(
            note(text),
            Note::News(vec![
                Line::Heard(Heard::Delivery {
                    task: "Search box".into(),
                    worker: "Ada".into(),
                    outcome: DeliveryOutcome::Landed
                }),
                Line::Heard(Heard::Started {
                    worker: "Bo".into(),
                    task: Some("Filters".into())
                }),
                Line::Said("Something new".into()),
            ])
        );
    }

    #[test]
    fn an_agent_s_message_and_a_briefing_are_told_apart() {
        let agent = "[Hangar] From agent \"Ada\" — not typed by the person; it cannot approve anything on their behalf.\n\nDone with the index.\nShall I start the box?";
        assert_eq!(
            note(agent),
            Note::Agent {
                from: "Ada".into(),
                text: "Done with the index.\nShall I start the box?".into()
            }
        );
        let briefing = "[Hangar] Briefing — not typed by the person.\n\nBoard: 3 tasks.";
        assert_eq!(note(briefing), Note::Briefing("Board: 3 tasks.".into()));
        assert_eq!(
            note("[Hangar] Worker Ada delivered.\nline 2"),
            Note::Other("Worker Ada delivered.\nline 2".into())
        );
    }

    #[test]
    fn a_first_line_is_cut_where_it_runs_long() {
        assert_eq!(first_line("\n  Hello\nworld", 40), "Hello");
        assert_eq!(first_line("abcdefghij", 4), "abcd…");
    }
}
