//! The lines Hangar writes to an orchestrator about its workspace
//! (`describe_event`), read back for the person.
//!
//! What the orchestrator is told is worded for the model, in English, with
//! what to do next. The chat view shows the same news to the person as short
//! lines in their own language. It reads them from the conversation, where
//! the messages are text: each line `describe_event` writes is read back here
//! into what it says (`Heard`), next to the code that writes it, and a test
//! writes each kind of event and reads it back, so a change of wording on one
//! side cannot leave the other behind.

use super::events::DeliveryOutcome;

/// What one line about the workspace says. A task is named by its title,
/// or by its id when the line gave none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Delivery {
        task: String,
        worker: String,
        outcome: DeliveryOutcome,
    },
    NeedsYou {
        task: String,
        reason: String,
    },
    Blocked {
        task: String,
        /// `None`: the person.
        by: Option<String>,
        reason: String,
    },
    Unblocked {
        task: String,
    },
    RecheckDue {
        task: String,
    },
    Commented {
        task: String,
        /// `None`: the person.
        by: Option<String>,
        text: String,
    },
    Moved {
        task: String,
        from: String,
        to: String,
    },
    Started {
        worker: String,
        task: Option<String>,
    },
    Stopped {
        worker: String,
        task: Option<String>,
    },
    Exited {
        worker: String,
        task: Option<String>,
    },
    Waiting {
        worker: String,
        minutes: u64,
        task: Option<String>,
    },
    Idle {
        worker: String,
        minutes: u64,
        task: Option<String>,
    },
}

/// How each outcome reads after "Delivery of … by …": `describe_event`'s
/// own words, the longest first so that none is taken for another.
const OUTCOMES: [(DeliveryOutcome, &str); 9] = [
    (
        DeliveryOutcome::Stale,
        "was not landed: the worker committed after it was checked and reviewed",
    ),
    (
        DeliveryOutcome::ChangesRequested,
        "came back from review with changes requested",
    ),
    (
        DeliveryOutcome::Ready,
        "passed its check and review; it waits for the person to land it",
    ),
    (
        DeliveryOutcome::Done,
        "is in: its changes are in the workspace's folder",
    ),
    (DeliveryOutcome::Landed, "landed: merged on the branch"),
    (DeliveryOutcome::Conflict, "hit a merge conflict"),
    (DeliveryOutcome::CheckFailed, "failed the check"),
    (
        DeliveryOutcome::Dirty,
        "was held back by uncommitted changes",
    ),
    (DeliveryOutcome::Error, "failed"),
];

/// Who the line says did something: `None` for the person.
fn who(name: &str) -> Option<String> {
    (name != "The person").then(|| name.to_string())
}

/// A task as `task_ref` writes it: `"title" [id]`, or `[id]`; the title,
/// else the id.
fn task_of(text: &str) -> Option<String> {
    let text = text.trim();
    let open = text.rfind(" [").map(|at| at + 1).or_else(|| {
        // `[id]` alone.
        text.starts_with('[').then_some(0)
    })?;
    let id = text[open..].strip_prefix('[')?.strip_suffix(']')?;
    let title = text[..open].trim();
    match title.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        Some(title) if !title.is_empty() => Some(title.to_string()),
        _ if title.is_empty() && !id.is_empty() => Some(id.to_string()),
        _ => None,
    }
}

/// `" on <task>"` at the end of `text`, split off: what is before, and the
/// task when there is one.
fn on_task(text: &str) -> (String, Option<String>) {
    match text.split_once(" on ") {
        Some((before, task)) => match task_of(task) {
            Some(task) => (before.to_string(), Some(task)),
            None => (text.to_string(), None),
        },
        None => (text.to_string(), None),
    }
}

/// `"12 min"` as `minutes` writes it.
fn minutes_of(text: &str) -> Option<u64> {
    text.trim().strip_suffix(" min")?.trim().parse().ok()
}

/// One line of an update, as `describe_event` writes it, without its
/// leading "- "; `None` for a line it did not write.
pub fn read_line(line: &str) -> Option<Heard> {
    let line = line.trim().trim_start_matches("- ").trim();
    if let Some(rest) = line.strip_prefix("Delivery of ") {
        let (outcome, at) = OUTCOMES.iter().find_map(|(outcome, words)| {
            rest.find(&format!(" {words}")).map(|at| (*outcome, at))
        })?;
        let (task, worker) = rest[..at].rsplit_once(" by ")?;
        return Some(Heard::Delivery {
            task: task_of(task)?,
            worker: worker.to_string(),
            outcome,
        });
    }
    if let Some((task, reason)) = line.split_once(" needs the person: ") {
        return Some(Heard::NeedsYou {
            task: task_of(task)?,
            reason: reason.to_string(),
        });
    }
    if let Some(head) = line.strip_suffix("; the task is still assigned to it")
        && let Some((worker, rest)) = head.split_once("'s session ended")
    {
        return Some(Heard::Exited {
            worker: worker.to_string(),
            task: on_task(rest).1,
        });
    }
    if let Some((head, _)) = line
        .split_once(": a permission or a question in its pane, which only the person can answer")
        && let Some((worker, rest)) = head.split_once(" has been waiting for the person for ")
    {
        let (time, task) = on_task(rest);
        return Some(Heard::Waiting {
            worker: worker.to_string(),
            minutes: minutes_of(&time)?,
            task,
        });
    }
    if let Some((worker, rest)) = line.split_once(" ended its turn ")
        && let Some((time, rest)) = rest.split_once(" ago without delivering")
    {
        return Some(Heard::Idle {
            worker: worker.to_string(),
            minutes: minutes_of(time)?,
            task: on_task(rest).1,
        });
    }
    if let Some((head, reason)) =
        line.split_once(": its recheck day passed and it stays blocked until the person lifts it (")
        && reason.ends_with(')')
    {
        return Some(Heard::RecheckDue {
            task: task_of(head)?,
        });
    }
    if let Some((by, rest)) = line.split_once(" commented on ")
        && let Some((task, text)) = rest.split_once(": \"")
    {
        return Some(Heard::Commented {
            task: task_of(task)?,
            by: who(by),
            text: text.strip_suffix('"').unwrap_or(text).to_string(),
        });
    }
    if let Some((by, rest)) = line.split_once(" blocked ")
        && let Some((task, reason)) = rest.split_once(": ")
        && let Some(task) = task_of(task)
    {
        let reason = reason
            .strip_suffix(" (only the person can unblock it)")
            .unwrap_or(reason);
        return Some(Heard::Blocked {
            task,
            by: who(by),
            reason: reason.to_string(),
        });
    }
    if let Some((task, _)) = line.split_once(" unblocked: ")
        && let Some(task) = task_of(task)
    {
        return Some(Heard::Unblocked { task });
    }
    if let Some((task, rest)) = line.split_once(" moved ")
        && let Some(task) = task_of(task)
        && let Some((from, to)) = rest.split_once(" → ")
    {
        let to = to.split(" (by ").next().unwrap_or(to);
        return Some(Heard::Moved {
            task,
            from: from.to_string(),
            to: to.to_string(),
        });
    }
    if let Some((worker, rest)) = line.split_once(" was stopped") {
        return Some(Heard::Stopped {
            worker: worker.to_string(),
            task: on_task(rest).1,
        });
    }
    if let Some((worker, rest)) = line.split_once(" started")
        && (rest.is_empty() || rest.starts_with(" on "))
    {
        return Some(Heard::Started {
            worker: worker.to_string(),
            task: on_task(rest).1,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::events::{OrchestratorEvent as E, describe_event};
    use super::*;

    fn titles(id: &str) -> Option<String> {
        (id == "t1").then(|| "Search box".to_string())
    }

    fn read(event: E) -> Option<Heard> {
        let line = describe_event(&event, &titles);
        read_line(&format!("- {line}"))
    }

    fn task() -> String {
        "Search box".into()
    }

    #[test]
    fn every_delivery_reads_back_with_its_outcome() {
        for (outcome, _) in OUTCOMES {
            for detail in ["", "tests: 3 failed"] {
                let heard = read(E::Delivery {
                    task_id: "t1".into(),
                    title: "Search box".into(),
                    worker: "Ada".into(),
                    outcome,
                    detail: detail.into(),
                });
                assert_eq!(
                    heard,
                    Some(Heard::Delivery {
                        task: task(),
                        worker: "Ada".into(),
                        outcome
                    }),
                    "{outcome:?} {detail:?}"
                );
            }
        }
    }

    #[test]
    fn the_tasks_news_reads_back() {
        let t = || ("t1".to_string(), "Search box".to_string());
        assert_eq!(
            read(E::TaskNeedsYou {
                task_id: t().0,
                title: t().1,
                reason: "pick a colour".into()
            }),
            Some(Heard::NeedsYou {
                task: task(),
                reason: "pick a colour".into()
            })
        );
        for (by, human_only, who) in [("user", true, None), ("Ada", false, Some("Ada"))] {
            assert_eq!(
                read(E::TaskBlocked {
                    task_id: t().0,
                    title: t().1,
                    by: by.into(),
                    reason: "waits on the API".into(),
                    human_only,
                }),
                Some(Heard::Blocked {
                    task: task(),
                    by: who.map(str::to_string),
                    reason: "waits on the API".into()
                })
            );
        }
        assert_eq!(
            read(E::TaskUnblocked {
                task_id: t().0,
                title: t().1,
                how: "by Ada".into()
            }),
            Some(Heard::Unblocked { task: task() })
        );
        assert_eq!(
            read(E::TaskRecheckDue {
                task_id: t().0,
                title: t().1,
                reason: "vendor".into()
            }),
            Some(Heard::RecheckDue { task: task() })
        );
        assert_eq!(
            read(E::TaskCommented {
                task_id: t().0,
                title: t().1,
                by: "user".into(),
                text: "make it \"blue\"".into()
            }),
            Some(Heard::Commented {
                task: task(),
                by: None,
                text: "make it \"blue\"".into()
            })
        );
        for by in [None, Some("Ada".to_string())] {
            assert_eq!(
                read(E::TaskMoved {
                    task_id: t().0,
                    title: t().1,
                    from: "todo".into(),
                    to: "doing".into(),
                    by
                }),
                Some(Heard::Moved {
                    task: task(),
                    from: "todo".into(),
                    to: "doing".into()
                })
            );
        }
    }

    #[test]
    fn the_workers_news_reads_back_with_or_without_a_task() {
        for (task_id, on) in [
            (Some("t1"), Some(task())),
            (None, None),
            (Some("x9"), Some("x9".into())),
        ] {
            let task_id = task_id.map(str::to_string);
            let worker = || "Ada".to_string();
            assert_eq!(
                read(E::WorkerStarted {
                    worker: worker(),
                    task_id: task_id.clone()
                }),
                Some(Heard::Started {
                    worker: worker(),
                    task: on.clone()
                })
            );
            assert_eq!(
                read(E::WorkerStopped {
                    worker: worker(),
                    task_id: task_id.clone()
                }),
                Some(Heard::Stopped {
                    worker: worker(),
                    task: on.clone()
                })
            );
            assert_eq!(
                read(E::WorkerExited {
                    worker: worker(),
                    task_id: task_id.clone()
                }),
                Some(Heard::Exited {
                    worker: worker(),
                    task: on.clone()
                })
            );
            assert_eq!(
                read(E::WorkerWaiting {
                    worker: worker(),
                    task_id: task_id.clone(),
                    for_ms: 7 * 60_000
                }),
                Some(Heard::Waiting {
                    worker: worker(),
                    minutes: 7,
                    task: on.clone()
                })
            );
            assert_eq!(
                read(E::WorkerIdle {
                    worker: worker(),
                    task_id: task_id.clone(),
                    for_ms: 12 * 60_000
                }),
                Some(Heard::Idle {
                    worker: worker(),
                    minutes: 12,
                    task: on.clone()
                })
            );
        }
    }

    #[test]
    fn a_line_hangar_did_not_write_is_not_read() {
        assert_eq!(read_line("Every task on the board is done."), None);
        assert_eq!(read_line("Ready to start: \"A\" [t2]."), None);
        assert_eq!(read_line("Hello there"), None);
    }
}
