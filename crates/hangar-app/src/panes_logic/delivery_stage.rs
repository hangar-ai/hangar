//! Where each task's delivery stands, for its worker's pane and the tasks
//! panel: queued (and its place), merging, checking (since when, alone or in
//! a batch), reviewing, landing, then how it ended, for a few seconds.
//!
//! Fed by core's `delivery:stages` (`CoreEvent::DeliveryStages`), which comes
//! each time a delivery joins its queue, changes stage or ends: nothing here
//! polls. The only clock is the `now` handed in. The checking chip's elapsed
//! time ticks once a second while one is on screen (`panes_logic`'s
//! subscription), and an outcome wakes the window once more when it is due
//! to go: nothing else wakes it.

use std::collections::HashMap;

use hangar_core::delivery::{Kind, Stages};
use hangar_i18n::{Key, Locale, Translator, args};

use crate::state::live::EpochMs;

/// How long an outcome stays on the pane once the delivery ended.
pub const OUTCOME_SHOW_MS: EpochMs = 6_000;

/// One delivery's stage, as a chip says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStage {
    /// Waiting its turn: `place` 1 is the next one to go.
    Queued {
        place: u32,
    },
    Merging,
    /// Its check runs since `since`; `batch` deliveries share it.
    Checking {
        since: EpochMs,
        batch: Option<u32>,
    },
    Reviewing,
    Landing,
    /// Over, with how it went; shown until `OUTCOME_SHOW_MS` after.
    Ended(Kind),
}

impl DeliveryStage {
    /// Still under way, rather than over.
    pub fn running(self) -> bool {
        !matches!(self, DeliveryStage::Ended(_))
    }

    /// Ended badly: the chip takes the danger colour.
    pub fn failed(self) -> bool {
        matches!(self, DeliveryStage::Ended(kind) if !matches!(kind, Kind::Landed | Kind::Ready))
    }
}

/// The stages of the deliveries under way, and the outcomes still shown.
#[derive(Debug, Default)]
pub struct DeliveryStages {
    running: HashMap<String, DeliveryStage>,
    /// The deliveries that ended, with when.
    ended: HashMap<String, (Kind, EpochMs)>,
}

impl DeliveryStages {
    /// Core said where the deliveries stand now (`running`, in the order
    /// they joined their queues) and, maybe, how one ended.
    pub fn apply(&mut self, stages: &Stages, now: EpochMs) {
        let mut waiting: HashMap<&str, u32> = HashMap::new();
        self.running = stages
            .running
            .iter()
            .filter_map(|entry| {
                let stage = match entry.stage.as_str() {
                    "queued" => {
                        let ahead = waiting.entry(entry.queue.as_str()).or_default();
                        *ahead += 1;
                        DeliveryStage::Queued { place: *ahead }
                    }
                    "merging" => DeliveryStage::Merging,
                    "checking" => DeliveryStage::Checking {
                        since: entry.since,
                        batch: entry.batch.filter(|&size| size > 1),
                    },
                    "reviewing" => DeliveryStage::Reviewing,
                    "landing" => DeliveryStage::Landing,
                    // A stage this version does not know says nothing.
                    _ => return None,
                };
                Some((entry.task_id.clone(), stage))
            })
            .collect();
        if let Some(ended) = &stages.ended {
            self.ended
                .insert(ended.task_id.clone(), (ended.outcome, now));
        }
        // Submitted again: the new delivery is what the pane says.
        let running = &self.running;
        self.ended.retain(|task, _| !running.contains_key(task));
        self.prune(now);
    }

    /// Where `task`'s delivery stands at `now`, if it runs or just ended.
    pub fn stage(&self, task: &str, now: EpochMs) -> Option<DeliveryStage> {
        if let Some(stage) = self.running.get(task) {
            return Some(*stage);
        }
        self.ended
            .get(task)
            .filter(|(_, at)| now < at + OUTCOME_SHOW_MS)
            .map(|(kind, _)| DeliveryStage::Ended(*kind))
    }

    /// The tasks whose delivery is being checked: their chip's time moves.
    pub fn checking(&self) -> impl Iterator<Item = &str> {
        self.running
            .iter()
            .filter(|(_, stage)| matches!(stage, DeliveryStage::Checking { .. }))
            .map(|(task, _)| task.as_str())
    }

    /// When the soonest outcome shown is due to go.
    pub fn next_expiry(&self) -> Option<EpochMs> {
        self.ended
            .values()
            .map(|(_, at)| at + OUTCOME_SHOW_MS)
            .min()
    }

    /// Forgets the outcomes shown long enough. Whether one went.
    pub fn prune(&mut self, now: EpochMs) -> bool {
        let before = self.ended.len();
        self.ended.retain(|_, (_, at)| now < *at + OUTCOME_SHOW_MS);
        self.ended.len() != before
    }
}

/// A duration as a clock shows it: `3:12`, `1:02:03`.
pub fn elapsed(ms: EpochMs) -> String {
    let seconds = ms / 1000;
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// A place in a queue, as the language writes it: `2nd`, `2e`.
pub fn ordinal(locale: Locale, n: u32) -> String {
    match locale {
        Locale::Fr => {
            if n == 1 {
                "1er".into()
            } else {
                format!("{n}e")
            }
        }
        _ => {
            let suffix = match (n % 10, n % 100) {
                (_, 11..=13) => "th",
                (1, _) => "st",
                (2, _) => "nd",
                (3, _) => "rd",
                _ => "th",
            };
            format!("{n}{suffix}")
        }
    }
}

/// What the chip says; `short` where its header has little room ("Batch of
/// 3 · 0:12" rather than "Checking in a batch of 3 · 0:12").
pub fn label(stage: DeliveryStage, now: EpochMs, tr: Translator, short: bool) -> String {
    match stage {
        DeliveryStage::Queued { place } => tr
            .t_with(
                Key::PaneDeliveryQueued,
                &args!("place" => ordinal(tr.locale(), place)),
            )
            .into_owned(),
        DeliveryStage::Merging => tr.t(Key::PaneDeliveryMerging).into_owned(),
        DeliveryStage::Checking { since, batch } => {
            let time = elapsed(now.saturating_sub(since));
            match batch {
                Some(size) => tr
                    .t_with(
                        if short {
                            Key::PaneDeliveryCheckingBatchShort
                        } else {
                            Key::PaneDeliveryCheckingBatch
                        },
                        &args!("size" => size.to_string(), "elapsed" => time),
                    )
                    .into_owned(),
                None => tr
                    .t_with(Key::PaneDeliveryChecking, &args!("elapsed" => time))
                    .into_owned(),
            }
        }
        DeliveryStage::Reviewing => tr.t(Key::PaneDeliveryReviewing).into_owned(),
        DeliveryStage::Landing => tr.t(Key::PaneDeliveryLanding).into_owned(),
        DeliveryStage::Ended(kind) => tr
            .t(match kind {
                Kind::Landed => Key::PaneDeliveryLanded,
                Kind::Ready => Key::PaneDeliveryReady,
                Kind::CheckFailed => Key::PaneDeliveryCheckFailed,
                Kind::Conflict => Key::PaneDeliveryConflict,
                Kind::ChangesRequested => Key::PaneDeliveryChangesRequested,
                Kind::Dirty | Kind::Stale | Kind::Error => Key::PaneDeliveryFailed,
            })
            .into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hangar_core::delivery::{Running, StageEnd};

    fn running(task: &str, queue: &str, stage: &str, since: u64, batch: Option<u32>) -> Running {
        Running {
            task_id: task.into(),
            queue: queue.into(),
            stage: stage.into(),
            since,
            batch,
        }
    }

    fn stages(running: Vec<Running>, ended: Option<(&str, Kind)>) -> Stages {
        Stages {
            running,
            ended: ended.map(|(task, outcome)| StageEnd {
                task_id: task.into(),
                outcome,
            }),
        }
    }

    const EN: Translator = Translator::new(Locale::En);
    const FR: Translator = Translator::new(Locale::Fr);

    #[test]
    fn each_stage_has_its_words() {
        let now = 1_000_000;
        let say = |stage| label(stage, now, EN, false);
        assert_eq!(say(DeliveryStage::Queued { place: 2 }), "Queued (2nd)");
        assert_eq!(say(DeliveryStage::Merging), "Merging");
        assert_eq!(
            say(DeliveryStage::Checking {
                since: now - 192_000,
                batch: None
            }),
            "Checking · 3:12"
        );
        assert_eq!(say(DeliveryStage::Reviewing), "Reviewing");
        assert_eq!(say(DeliveryStage::Landing), "Landing");
        assert_eq!(say(DeliveryStage::Ended(Kind::Landed)), "Landed ✓");
        assert_eq!(
            say(DeliveryStage::Ended(Kind::CheckFailed)),
            "Check failed ✗"
        );
        assert_eq!(say(DeliveryStage::Ended(Kind::Error)), "Delivery failed ✗");
        assert!(label(DeliveryStage::Queued { place: 1 }, now, FR, false).contains("1er"));
        assert!(label(DeliveryStage::Merging, now, FR, false).contains("Fusion"));
    }

    #[test]
    fn a_batch_says_how_many_share_the_check() {
        let mut book = DeliveryStages::default();
        book.apply(
            &stages(
                vec![
                    running("a", "repo", "checking", 0, Some(3)),
                    running("b", "repo", "checking", 0, Some(1)),
                ],
                None,
            ),
            0,
        );
        let a = book.stage("a", 5_000).unwrap();
        assert_eq!(
            label(a, 5_000, EN, false),
            "Checking in a batch of 3 · 0:05"
        );
        assert_eq!(label(a, 5_000, EN, true), "Batch of 3 · 0:05");
        // A batch of one is a check of its own.
        assert_eq!(
            label(book.stage("b", 5_000).unwrap(), 5_000, EN, true),
            "Checking · 0:05"
        );
    }

    #[test]
    fn elapsed_reads_as_a_clock() {
        assert_eq!(elapsed(0), "0:00");
        assert_eq!(elapsed(59_999), "0:59");
        assert_eq!(elapsed(192_000), "3:12");
        assert_eq!(elapsed(3_723_000), "1:02:03");
    }

    #[test]
    fn places_read_as_the_language_writes_them() {
        let en: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23]
            .into_iter()
            .map(|n| ordinal(Locale::En, n))
            .collect();
        assert_eq!(
            en,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd"
            ]
        );
        assert_eq!(ordinal(Locale::Fr, 1), "1er");
        assert_eq!(ordinal(Locale::Fr, 2), "2e");
    }

    #[test]
    fn places_count_in_each_queue_the_ones_waiting_ahead() {
        let mut book = DeliveryStages::default();
        book.apply(
            &stages(
                vec![
                    running("busy", "repo", "checking", 0, None),
                    running("first", "repo", "queued", 0, None),
                    running("other", "elsewhere", "queued", 0, None),
                    running("second", "repo", "queued", 0, None),
                ],
                None,
            ),
            0,
        );
        assert_eq!(
            book.stage("first", 0),
            Some(DeliveryStage::Queued { place: 1 })
        );
        assert_eq!(
            book.stage("second", 0),
            Some(DeliveryStage::Queued { place: 2 })
        );
        assert_eq!(
            book.stage("other", 0),
            Some(DeliveryStage::Queued { place: 1 })
        );
    }

    #[test]
    fn nothing_is_shown_without_a_delivery() {
        let mut book = DeliveryStages::default();
        assert_eq!(book.stage("t", 0), None);
        book.apply(
            &stages(vec![running("x", "repo", "merging", 0, None)], None),
            0,
        );
        assert_eq!(book.stage("t", 0), None);
        // An unknown stage says nothing either.
        book.apply(
            &stages(vec![running("t", "repo", "warping", 0, None)], None),
            0,
        );
        assert_eq!(book.stage("t", 0), None);
        assert_eq!(book.next_expiry(), None);
    }

    #[test]
    fn an_outcome_shows_for_a_while_then_goes() {
        let mut book = DeliveryStages::default();
        book.apply(
            &stages(vec![running("t", "repo", "landing", 0, None)], None),
            0,
        );
        book.apply(&stages(vec![], Some(("t", Kind::Landed))), 10_000);
        assert_eq!(
            book.stage("t", 10_000),
            Some(DeliveryStage::Ended(Kind::Landed))
        );
        assert_eq!(book.next_expiry(), Some(10_000 + OUTCOME_SHOW_MS));
        assert_eq!(book.stage("t", 10_000 + OUTCOME_SHOW_MS), None);
        assert!(book.prune(10_000 + OUTCOME_SHOW_MS));
        assert_eq!(book.next_expiry(), None);
    }

    #[test]
    fn a_new_delivery_replaces_the_outcome() {
        let mut book = DeliveryStages::default();
        book.apply(&stages(vec![], Some(("t", Kind::CheckFailed))), 0);
        assert!(book.stage("t", 0).unwrap().failed());
        book.apply(
            &stages(vec![running("t", "repo", "queued", 0, None)], None),
            1,
        );
        assert_eq!(book.stage("t", 1), Some(DeliveryStage::Queued { place: 1 }));
        assert_eq!(book.next_expiry(), None);
    }

    #[test]
    fn only_checks_make_the_time_move() {
        let mut book = DeliveryStages::default();
        book.apply(
            &stages(
                vec![
                    running("q", "repo", "queued", 0, None),
                    running("r", "repo", "reviewing", 0, None),
                ],
                None,
            ),
            0,
        );
        assert_eq!(book.checking().count(), 0);
        book.apply(
            &stages(vec![running("c", "repo", "checking", 0, None)], None),
            0,
        );
        assert_eq!(book.checking().collect::<Vec<_>>(), ["c"]);
    }
}
