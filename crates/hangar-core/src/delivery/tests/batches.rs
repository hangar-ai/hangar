//! The merge queue (docs/native/DELIVERIES.md): deliveries that wait
//! together are merged in turn, checked once, halved while red, and main only
//! ever moves to a tree the check passed on.

use super::*;
use crate::delivery::batch::{self, Next, Settle, Ticket};

/// A worker's worktree on a branch `name` of its own, from main, with `file`
/// committed holding `text`, and its delivery.
fn member(repo: &Repo, name: &str, file: &str, text: &str, check: &str) -> Delivery {
    let worktree = repo.root.join(name);
    sh(
        &repo.main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            name,
            &worktree.to_string_lossy(),
            "main",
        ],
    );
    write(&worktree, file, text);
    commit(&worktree, &format!("Add {file}"));
    let mut request = request(repo);
    request.worktree = worktree.to_string_lossy().into_owned();
    request.branch = name.into();
    request.task = TaskBrief {
        id: format!("{}-{name}", repo.task),
        title: format!("Add {file}"),
        description: format!("Task for {name}: add {file}."),
        owns: vec![file.into()],
    };
    request.check_command = Some(check.into());
    request
}

/// A check that counts its runs in `log` and fails when `bad.rs` is there.
fn counting(repo: &Repo) -> (String, PathBuf) {
    let log = repo.root.join("check-runs.log");
    let line = if cfg!(windows) {
        format!(
            "echo x>> \"{}\" && if exist bad.rs (echo bad.rs is here && exit 1)",
            log.display()
        )
    } else {
        format!(
            "echo x >> '{}' && if [ -e bad.rs ]; then echo bad.rs is here; exit 1; fi",
            log.display()
        )
    };
    (line, log)
}

fn runs(log: &Path) -> usize {
    fs::read_to_string(log).unwrap_or_default().lines().count()
}

fn done(settle: &Settle) -> &Outcome {
    match settle {
        Settle::Done(outcome) => outcome,
        Settle::Alone => panic!("set aside"),
    }
}

/// Main's tree passed the check: it is one the check ran on.
fn checked(repo: &Repo, request: &Delivery, line: &str) -> bool {
    let tree = head(&repo.main, "main^{tree}");
    let queue = key(&common_dir(&repo.main).unwrap().to_string_lossy());
    passed_before(request, &queue, &tree, line)
}

/// Refuses the deliveries whose task names `marker`, approves the rest.
struct Refuses(&'static str);

impl Reviewer for Refuses {
    fn review(&self, _: &Path, _: &str, prompt: &str, _: Option<&str>) -> Result<Verdict, String> {
        if prompt.contains(self.0) {
            Ok(Verdict {
                approve: false,
                blocking: vec![Blocking {
                    file: "x".into(),
                    line: None,
                    problem: "no".into(),
                }],
                notes: String::new(),
            })
        } else {
            Ok(approve())
        }
    }
}

#[test]
fn a_green_batch_lands_every_member_in_order_on_one_check() {
    let repo = repo("batch-green");
    let (line, log) = counting(&repo);
    let members = vec![
        member(&repo, "a", "a.rs", "a\n", &line),
        member(&repo, "b", "b.rs", "b\n", &line),
        member(&repo, "c", "c.rs", "c\n", &line),
    ];
    let reviewer = fake(Ok(approve()));

    let settled = batch::run(&members, &reviewer, &Limits::default());

    assert_eq!(runs(&log), 1, "one check for the three");
    let outcomes: Vec<&Outcome> = settled.iter().map(done).collect();
    for (outcome, request) in outcomes.iter().zip(&members) {
        assert_eq!(outcome.outcome, Kind::Landed, "{}", outcome.detail);
        let merge = outcome.merge.clone().unwrap();
        // The worker reads what a delivery on its own would have said.
        assert!(
            outcome
                .detail
                .starts_with(&landed(request, &merge, outcome.landed_in.as_deref())),
            "{}",
            outcome.detail
        );
        assert!(outcome.detail.contains("checked in a batch of 3"));
        assert_eq!(outcome.changed, request.task.owns, "its own change only");
        assert!(outcome.check.as_ref().unwrap().passed);
        assert!(outcome.review.is_some());
    }
    // Each one merged on the one before it, main at the end of the chain.
    assert_eq!(
        outcomes[0].base.as_deref(),
        Some(head(&repo.main, "main~3").as_str())
    );
    assert_eq!(outcomes[1].base, outcomes[0].merge);
    assert_eq!(outcomes[2].base, outcomes[1].merge);
    assert_eq!(
        outcomes[2].merge.as_deref(),
        Some(head(&repo.main, "main").as_str())
    );
    let subjects = sh(
        &repo.main,
        &["log", "--first-parent", "--format=%s", "-3", "main"],
    );
    assert_eq!(
        subjects.lines().collect::<Vec<_>>(),
        // The first one on main is a fast-forward, as on its own.
        ["Merge branch 'c'", "Merge branch 'b'", "Add a.rs"]
    );
    for file in ["a.rs", "b.rs", "c.rs"] {
        assert!(repo.main.join(file).is_file(), "{file} landed");
    }
    assert!(checked(&repo, &members[0], &line));
    // The reviewer read each one's own diff, told about the ones before it.
    let prompts = reviewer.prompts.borrow();
    assert_eq!(prompts.len(), 3);
    assert!(prompts[1].contains("+b") && !prompts[1].contains("+a\n"));
    assert!(prompts[1].contains("with a merged before it"));
    assert!(!prompts[0].contains("merged before it"));
}

#[test]
fn a_failing_member_is_found_by_halving_and_only_it_fails() {
    let repo = repo("batch-red");
    let (line, log) = counting(&repo);
    let members = vec![
        member(&repo, "a", "a.rs", "a\n", &line),
        member(&repo, "b", "b.rs", "b\n", &line),
        member(&repo, "c", "bad.rs", "bad\n", &line),
        member(&repo, "d", "d.rs", "d\n", &line),
    ];
    let before = head(&repo.main, "main");

    let settled = batch::run(&members, &fake(Ok(approve())), &Limits::default());

    let kinds: Vec<Kind> = settled.iter().map(|s| done(s).outcome).collect();
    assert_eq!(
        kinds,
        [Kind::Landed, Kind::Landed, Kind::CheckFailed, Kind::Landed]
    );
    let failed = done(&settled[2]);
    let check = failed.check.clone().unwrap();
    assert!(
        !check.passed && check.tail.contains("bad.rs is here"),
        "{check:?}"
    );
    assert!(
        failed.detail.starts_with(&check_failed(
            &members[2],
            &line,
            &check,
            &Limits::default()
        )),
        "the same words as on its own: {}",
        failed.detail
    );
    assert_eq!(failed.changed, vec!["bad.rs".to_string()]);
    // The four (red), a + b (green), a + b + c (red: c breaks it), then d
    // on a + b.
    assert_eq!(runs(&log), 4);
    assert!(!repo.main.join("bad.rs").exists());
    for file in ["a.rs", "b.rs", "d.rs"] {
        assert!(repo.main.join(file).is_file(), "{file} landed");
    }
    assert_ne!(head(&repo.main, "main"), before);
    assert!(checked(&repo, &members[0], &line));
    assert_no_scratch_but(&repo, 4);
}

#[test]
fn a_failing_first_member_leaves_the_rest_checked_once_together() {
    let repo = repo("batch-red-first");
    let (line, log) = counting(&repo);
    let members = vec![
        member(&repo, "a", "bad.rs", "bad\n", &line),
        member(&repo, "b", "b.rs", "b\n", &line),
        member(&repo, "c", "c.rs", "c\n", &line),
        member(&repo, "d", "d.rs", "d\n", &line),
    ];

    let settled = batch::run(&members, &fake(Ok(approve())), &Limits::default());

    let kinds: Vec<Kind> = settled.iter().map(|s| done(s).outcome).collect();
    assert_eq!(
        kinds,
        [Kind::CheckFailed, Kind::Landed, Kind::Landed, Kind::Landed]
    );
    // The four, a + b, a alone (red: a breaks it), then b + c + d on main.
    assert_eq!(runs(&log), 4);
    assert!(!repo.main.join("bad.rs").exists());
    assert!(checked(&repo, &members[1], &line));
}

/// `assert_no_scratch` for a repository with `extra` more worktrees.
fn assert_no_scratch_but(repo: &Repo, extra: usize) {
    let listed = worktrees(&repo.main).unwrap();
    let checks = format!("{}/", key(&repo.root.join("checks").to_string_lossy()));
    let folders: Vec<_> = listed
        .iter()
        .filter(|(path, _, _)| key(path).starts_with(&checks))
        .collect();
    assert!(folders.len() <= 1, "{listed:?}");
    assert_eq!(listed.len(), 2 + extra + folders.len(), "{listed:?}");
    for (path, _, _) in folders {
        assert_eq!(sh(Path::new(path), &["status", "--porcelain"]), "");
    }
}

#[test]
fn a_member_that_conflicts_with_one_before_it_is_set_aside_and_gets_its_conflict_alone() {
    let repo = repo("batch-conflict");
    let (line, _) = counting(&repo);
    let members = vec![
        member(&repo, "a", "x.txt", "from a\n", &line),
        member(&repo, "b", "x.txt", "from b\n", &line),
        member(&repo, "c", "c.rs", "c\n", &line),
    ];

    let settled = batch::run(&members, &fake(Ok(approve())), &Limits::default());

    assert_eq!(done(&settled[0]).outcome, Kind::Landed);
    assert!(matches!(settled[1], Settle::Alone), "set aside");
    assert_eq!(done(&settled[2]).outcome, Kind::Landed);
    assert_eq!(read(&repo.main, "x.txt"), "from a\n");
    // Alone, on what the batch left: the conflict it would have had.
    let alone = deliver_now(&members[1], &Never);
    assert_eq!(alone.outcome, Kind::Conflict, "{}", alone.detail);
    assert_eq!(alone.files, vec!["x.txt".to_string()]);
}

#[test]
fn a_member_whose_branch_moved_during_the_batch_is_stale_and_the_rest_land_checked() {
    let repo = repo("batch-stale");
    let (line, log) = counting(&repo);
    let members = vec![
        member(&repo, "a", "a.rs", "a\n", &line),
        member(&repo, "b", "b.rs", "b\n", &line),
        member(&repo, "c", "c.rs", "c\n", &line),
    ];
    let b = PathBuf::from(&members[1].worktree);
    // The worker of b commits while b is reviewed.
    let reviewer = fake_with(vec![Ok(approve())], |call| {
        if call == 1 {
            write(&b, "b2.rs", "more\n");
            commit(&b, "More b");
        }
    });

    let settled = batch::run(&members, &reviewer, &Limits::default());

    assert_eq!(done(&settled[0]).outcome, Kind::Landed);
    let stale = done(&settled[1]);
    assert_eq!(stale.outcome, Kind::Stale, "{}", stale.detail);
    assert!(stale.detail.contains("delivery_submit again"));
    assert_eq!(done(&settled[2]).outcome, Kind::Landed);
    assert!(!repo.main.join("b.rs").exists() && !repo.main.join("b2.rs").exists());
    assert!(repo.main.join("a.rs").is_file() && repo.main.join("c.rs").is_file());
    // a + c was checked again before it landed.
    assert_eq!(runs(&log), 2);
    assert!(checked(&repo, &members[0], &line));
}

#[test]
fn a_refusal_in_a_batch_lands_the_others_only_on_a_tree_that_was_checked() {
    let repo = repo("batch-refused");
    let (line, log) = counting(&repo);
    let members = vec![
        member(&repo, "a", "a.rs", "a\n", &line),
        member(&repo, "b", "b.rs", "b\n", &line),
        member(&repo, "c", "c.rs", "c\n", &line),
    ];

    let settled = batch::run(&members, &Refuses("Task for b:"), &Limits::default());

    assert_eq!(done(&settled[0]).outcome, Kind::Landed);
    let refused = done(&settled[1]);
    assert_eq!(refused.outcome, Kind::ChangesRequested);
    assert!(
        refused.detail.starts_with(&changes_requested(
            &members[1],
            refused.review.as_ref().unwrap()
        )),
        "{}",
        refused.detail
    );
    assert_eq!(done(&settled[2]).outcome, Kind::Landed);
    assert!(!repo.main.join("b.rs").exists());
    // The tree a + c was never part of the first check: it ran again.
    assert_eq!(runs(&log), 2);
    assert!(checked(&repo, &members[0], &line));
}

#[test]
fn a_member_with_uncommitted_work_is_told_so_and_the_others_go_on() {
    let repo = repo("batch-dirty");
    let (line, _) = counting(&repo);
    let members = vec![
        member(&repo, "a", "a.rs", "a\n", &line),
        member(&repo, "b", "b.rs", "b\n", &line),
    ];
    write(Path::new(&members[1].worktree), "loose.rs", "x\n");

    let settled = batch::run(&members, &fake(Ok(approve())), &Limits::default());

    assert_eq!(done(&settled[0]).outcome, Kind::Landed);
    let dirty = done(&settled[1]);
    assert_eq!(dirty.outcome, Kind::Dirty);
    assert_eq!(dirty.files, vec!["loose.rs".to_string()]);
}

fn queue_of(request: &Delivery) -> String {
    key(&common_dir(Path::new(&request.worktree))
        .unwrap()
        .to_string_lossy())
}

#[test]
fn the_room_batches_what_waits_together_and_a_size_of_one_keeps_each_alone() {
    let repo = repo("batch-room");
    let (line, _) = counting(&repo);
    let a = member(&repo, "a", "a.rs", "a\n", &line);
    let b = member(&repo, "b", "b.rs", "b\n", &line);
    let queue = format!("{}-room", queue_of(&a));

    // A size of 1: each one alone, as before batches.
    let first = Ticket::join(&queue, &a);
    let second = Ticket::join(&queue, &b);
    assert!(matches!(first.next(1), Next::Alone));
    assert!(matches!(second.next(1), Next::Alone));
    drop((first, second));
    assert_eq!(batch::waiting(&queue), 0);

    // What does not land by itself, or has no check, goes alone too.
    let mut waits = b.clone();
    waits.land = false;
    let first = Ticket::join(&queue, &a);
    let second = Ticket::join(&queue, &waits);
    assert!(matches!(first.next(4), Next::Alone));
    drop((first, second));

    let first = Ticket::join(&queue, &a);
    let second = Ticket::join(&queue, &b);
    let Next::Batch(members) = first.next(4) else {
        panic!("not a batch");
    };
    assert_eq!(members.requests.len(), 2);
    assert_eq!(members.requests[0].branch, "a", "in the order they came");
    let outcome = |detail: &str| Settle::Done(Box::new(Outcome::new(Kind::Landed, detail.into())));
    first.settle(&members.seqs, vec![outcome("a"), outcome("b")]);
    let Next::Settled(mine) = second.next(4) else {
        panic!("the batch settled it");
    };
    assert_eq!(mine.detail, "b");
}

/// Approves everything, from any thread.
struct Approves;

impl Reviewer for Approves {
    fn review(&self, _: &Path, _: &str, _: &str, _: Option<&str>) -> Result<Verdict, String> {
        Ok(approve())
    }
}

#[cfg(unix)]
#[test]
fn deliveries_submitted_while_one_is_checked_wait_and_are_checked_together() {
    let repo = repo("batch-threads");
    let go = repo.root.join("go");
    let log = repo.root.join("check-runs.log");
    let line = format!(
        "while [ ! -e '{}' ]; do sleep 0.05; done; echo x >> '{}'",
        go.display(),
        log.display()
    );
    let requests = [
        member(&repo, "w", "w.rs", "w\n", &line),
        member(&repo, "a", "a.rs", "a\n", &line),
        member(&repo, "b", "b.rs", "b\n", &line),
        member(&repo, "c", "c.rs", "c\n", &line),
    ];
    let queue = queue_of(&requests[0]);
    let deliver_in = |request: Delivery| {
        std::thread::spawn(move || deliver(&request, &Approves, &Limits::default()))
    };
    let until = |what: &dyn Fn() -> bool| {
        let start = Instant::now();
        while !what() {
            assert!(start.elapsed() < Duration::from_secs(60), "timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    };

    let first = deliver_in(requests[0].clone());
    let w = requests[0].task.id.clone();
    until(&|| {
        delivery_running()
            .iter()
            .any(|run| run.task_id == w && run.stage == "checking")
    });
    let rest: Vec<_> = requests[1..].iter().cloned().map(deliver_in).collect();
    until(&|| batch::waiting(&queue) == 4);
    fs::write(&go, "").unwrap();

    assert_eq!(first.join().unwrap().outcome, Kind::Landed);
    for (thread, request) in rest.into_iter().zip(&requests[1..]) {
        let outcome = thread.join().unwrap();
        assert_eq!(outcome.outcome, Kind::Landed, "{}", outcome.detail);
        assert!(
            outcome.detail.contains("checked in a batch of 3"),
            "{}: {}",
            request.branch,
            outcome.detail
        );
    }
    // w alone, then a, b and c on one check.
    assert_eq!(runs(&log), 2);
    assert_eq!(batch::waiting(&queue), 0);
}

/// Five deliveries waiting together, the third one failing or not, on a check that
/// sleeps (`HANGAR_MEASURE_CHECK_SECS`, 60 by default): the wall-clock time
/// and the outcomes one at a time, then with batches of 4 and of 5. Only in
/// throwaway repositories; run it by hand:
/// `cargo test -p hangar-core --lib measure_a_queue -- --ignored --nocapture`.
#[cfg(unix)]
#[test]
#[ignore = "takes minutes: a measurement, run by hand"]
fn measure_a_queue_of_five_with_one_failing() {
    let secs: u64 = std::env::var("HANGAR_MEASURE_CHECK_SECS")
        .ok()
        .and_then(|secs| secs.parse().ok())
        .unwrap_or(60);
    let queue_of_five = |size: u32, failing: bool| {
        let repo = repo(&format!("measure-{size}-{failing}"));
        let log = repo.root.join("check-runs.log");
        let line = format!(
            "echo x >> '{}'; sleep {secs}; if [ -e bad.rs ]; then echo bad.rs is here; exit 1; fi",
            log.display()
        );
        let third = if failing { "bad.rs" } else { "c.rs" };
        let files = ["a.rs", "b.rs", third, "d.rs", "e.rs"];
        let requests: Vec<Delivery> = ["a", "b", "c", "d", "e"]
            .iter()
            .zip(files)
            .map(|(name, file)| {
                let mut request = member(&repo, name, file, "x\n", &line);
                request.batch_size = Some(size);
                request
            })
            .collect();
        let queue = queue_of(&requests[0]);
        // All five wait before the first one takes the turn.
        let turn = turn_of(&queue);
        let held = turn.lock().unwrap();
        let started = Instant::now();
        let threads: Vec<_> = requests
            .iter()
            .cloned()
            .map(|request| {
                std::thread::spawn(move || {
                    let outcome = deliver(&request, &Approves, &Limits::default());
                    (request.branch, outcome.outcome)
                })
            })
            .collect();
        while batch::waiting(&queue) < 5 {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(held);
        let mut outcomes: Vec<(String, Kind)> =
            threads.into_iter().map(|t| t.join().unwrap()).collect();
        outcomes.sort_by(|a, b| a.0.cmp(&b.0));
        let elapsed = started.elapsed().as_secs_f64();
        let checks = runs(&log);
        let mut landed: Vec<String> = sh(&repo.main, &["ls-files"])
            .lines()
            .map(str::to_string)
            .collect();
        landed.sort();
        eprintln!(
            "{} — batch size {size}: {elapsed:.1} s, {checks} checks of {secs} s, outcomes {outcomes:?}, main holds {landed:?}",
            if failing { "one failing" } else { "all green" },
        );
        (outcomes, landed, elapsed)
    };
    for failing in [true, false] {
        let (alone, alone_main, _) = queue_of_five(1, failing);
        for size in [4, 5] {
            let (batched, batched_main, _) = queue_of_five(size, failing);
            assert_eq!(batched, alone, "the same outcome for each delivery");
            assert_eq!(batched_main, alone_main, "the same files on main");
        }
    }
}
