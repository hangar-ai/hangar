//! The merge queue: deliveries that wait together are merged one after the
//! other on the target, checked once, and split in halves only when that
//! check fails. The target only ever moves to a tree the check passed on.
//! docs/native/DELIVERIES.md says why, and what happens in each case.

use super::*;

/* ── The room: who waits for the repository's turn ─────────────── */

struct Entry {
    seq: u64,
    request: Delivery,
    /// Left by the batch that took it.
    outcome: Option<Outcome>,
    /// In a batch running now, or gone alone.
    claimed: bool,
    /// Set aside by a batch (it conflicted with a delivery before it): it
    /// goes alone, on what the batch left.
    alone: bool,
}

fn rooms() -> &'static Mutex<HashMap<String, Vec<Entry>>> {
    static ROOMS: OnceLock<Mutex<HashMap<String, Vec<Entry>>>> = OnceLock::new();
    ROOMS.get_or_init(Default::default)
}

fn room_lock() -> std::sync::MutexGuard<'static, HashMap<String, Vec<Entry>>> {
    rooms()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A delivery's place in its repository's room, given up with it.
pub(super) struct Ticket {
    repo: String,
    seq: u64,
}

/// The deliveries a batch takes, in the order they were submitted.
pub(super) struct Members {
    pub seqs: Vec<u64>,
    pub requests: Vec<Delivery>,
}

pub(super) enum Next {
    /// A batch took it, and this is what came of it.
    Settled(Box<Outcome>),
    /// These wait with it and share its check: run them together.
    Batch(Members),
    /// Nothing to share a check with: the delivery goes on its own.
    Alone,
}

/// What a batch says of one of its members.
pub(super) enum Settle {
    Done(Box<Outcome>),
    /// Set aside: it goes on its own.
    Alone,
}

/// Can be taken into a batch: it lands by itself, it is not what waited for
/// the person, and there is a check to share.
fn batchable(request: &Delivery) -> bool {
    request.land && !request.reuse && check_line(request).is_some()
}

fn check_line(request: &Delivery) -> Option<&str> {
    request
        .check_command
        .as_deref()
        .map(str::trim)
        .filter(|line| !line.is_empty())
}

/// One check run says the same about both.
fn compatible(a: &Delivery, b: &Delivery) -> bool {
    let shell = |r: &Delivery| serde_json::to_string(&r.shell).unwrap_or_default();
    a.workspace == b.workspace
        && a.target == b.target
        && check_line(a) == check_line(b)
        && a.workspace_id == b.workspace_id
        && a.check_root == b.check_root
        && a.data_dir == b.data_dir
        && a.build_env == b.build_env
        && shell(a) == shell(b)
}

impl Ticket {
    pub(super) fn join(repo: &str, request: &Delivery) -> Ticket {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        room_lock()
            .entry(repo.to_string())
            .or_default()
            .push(Entry {
                seq,
                request: request.clone(),
                outcome: None,
                claimed: false,
                alone: false,
            });
        Ticket {
            repo: repo.to_string(),
            seq,
        }
    }

    /// Called with the repository's turn held: what this delivery does now.
    pub(super) fn next(&self, size: usize) -> Next {
        let mut rooms = room_lock();
        let room = rooms.entry(self.repo.clone()).or_default();
        let Some(me) = room.iter().position(|entry| entry.seq == self.seq) else {
            return Next::Alone;
        };
        if let Some(outcome) = room[me].outcome.take() {
            return Next::Settled(Box::new(outcome));
        }
        if room[me].alone || size < 2 || !batchable(&room[me].request) {
            room[me].claimed = true;
            return Next::Alone;
        }
        // Claimed without an outcome: the batch that took it never finished.
        room[me].claimed = false;
        let picks: Vec<usize> = (0..room.len())
            .filter(|&i| {
                let entry = &room[i];
                !entry.claimed
                    && !entry.alone
                    && entry.outcome.is_none()
                    && batchable(&entry.request)
                    && compatible(&entry.request, &room[me].request)
            })
            .take(size)
            .collect();
        if picks.len() < 2 {
            room[me].claimed = true;
            return Next::Alone;
        }
        let mut members = Members {
            seqs: vec![],
            requests: vec![],
        };
        for i in picks {
            room[i].claimed = true;
            members.seqs.push(room[i].seq);
            members.requests.push(room[i].request.clone());
        }
        Next::Batch(members)
    }

    /// Leaves each member's outcome for its own delivery to pick up.
    pub(super) fn settle(&self, seqs: &[u64], settled: Vec<Settle>) {
        let mut rooms = room_lock();
        let Some(room) = rooms.get_mut(&self.repo) else {
            return;
        };
        for (seq, settle) in seqs.iter().zip(settled) {
            if let Some(entry) = room.iter_mut().find(|entry| entry.seq == *seq) {
                entry.claimed = false;
                match settle {
                    Settle::Done(outcome) => entry.outcome = Some(*outcome),
                    Settle::Alone => entry.alone = true,
                }
            }
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut rooms = room_lock();
        if let Some(room) = rooms.get_mut(&self.repo) {
            room.retain(|entry| entry.seq != self.seq);
            if room.is_empty() {
                rooms.remove(&self.repo);
            }
        }
    }
}

/// How many deliveries wait in `repo`'s room, batched or not.
#[cfg(test)]
pub(super) fn waiting(repo: &str) -> usize {
    room_lock().get(repo).map_or(0, Vec::len)
}

/* ── A batch ───────────────────────────────────────────────────── */

/// A member merged in the chain: on `parent`, as `merge`.
#[derive(Clone)]
struct Link {
    member: usize,
    parent: String,
    merge: String,
}

enum Tested {
    /// The check that passed, or why it did not need to run.
    Green(Option<Check>, Option<String>),
    Red(Check),
    /// The check could not run at all.
    Broken(String),
}

struct Run<'a> {
    requests: &'a [Delivery],
    limits: &'a Limits,
    /// The first member's worktree: any worktree of the repository does.
    repo: PathBuf,
    repo_key: String,
    /// The check checkout.
    dir: PathBuf,
    line: String,
    /// The head of each member's branch, as it is merged.
    tips: Vec<String>,
    settled: Vec<Option<Settle>>,
    /// Trees the check failed on, with what it said: a half whose tree is
    /// one of them is red without running it again.
    red: HashMap<String, Check>,
    /// The last check that passed in this round: the one of the chain's end.
    green: Option<(Option<Check>, Option<String>)>,
    broken: Option<String>,
}

/// Merges, checks, reviews and lands `requests` together (at least two,
/// each `batchable` and `compatible` with the first), or says for each why
/// not. Holds the repository's turn throughout; the caller has it.
pub(super) fn run(requests: &[Delivery], reviewer: &dyn Reviewer, limits: &Limits) -> Vec<Settle> {
    let lead = &requests[0];
    let repo = PathBuf::from(&lead.worktree);
    let mut run = Run {
        requests,
        limits,
        repo_key: common_dir(&repo)
            .map(|dir| key(&dir.to_string_lossy()))
            .unwrap_or_default(),
        repo,
        dir: PathBuf::new(),
        line: check_line(lead).unwrap_or_default().to_string(),
        tips: vec![String::new(); requests.len()],
        settled: requests.iter().map(|_| None).collect(),
        red: HashMap::new(),
        green: None,
        broken: None,
    };
    run.all(reviewer);
    run.settled
        .into_iter()
        .map(|settled| settled.unwrap_or(Settle::Alone))
        .collect()
}

impl Run<'_> {
    fn done(&mut self, member: usize, outcome: Outcome) {
        if self.settled[member].is_none() {
            self.settled[member] = Some(Settle::Done(Box::new(outcome)));
        }
    }

    fn live(&self) -> Vec<usize> {
        (0..self.requests.len())
            .filter(|&member| self.settled[member].is_none())
            .collect()
    }

    fn stage(&self, member: usize, stage: &str) {
        set_stage(&self.requests[member].task.id, stage);
    }

    fn batch(&self, member: usize, batch: Option<usize>) {
        set_batch(&self.requests[member].task.id, batch.map(|n| n as u32));
    }

    /// "It was checked in a batch of 3 with a and b."
    fn note(&self, member: usize) -> String {
        let others: Vec<&str> = self
            .requests
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != member)
            .map(|(_, request)| request.branch.as_str())
            .collect();
        format!(
            "It was checked in a batch of {} with {}.",
            self.requests.len(),
            others.join(", ")
        )
    }

    /// An outcome for `link`'s member, with its own change.
    fn outcome(&self, link: &Link, kind: Kind, detail: String) -> Outcome {
        let request = &self.requests[link.member];
        let (changed, outside) = changes(&self.dir, request, &link.parent, &link.merge);
        let mut outcome = Outcome::new(kind, detail);
        outcome.changed = changed;
        outcome.outside_owns = outside;
        outcome.merge = Some(link.merge.clone());
        outcome.base = Some(link.parent.clone());
        if let Some((check, skipped)) = &self.green {
            outcome.check = check.clone();
            outcome.check_skipped = skipped.clone();
        }
        outcome
    }

    fn all(&mut self, reviewer: &dyn Reviewer) {
        let requests = self.requests;
        let lead = &requests[0];
        // 1. Only what is committed is delivered; each one replaces what of
        // its task waited for the person.
        for (member, request) in requests.iter().enumerate() {
            self.stage(member, "merging");
            let worktree = Path::new(&request.worktree);
            forget_tested(worktree, &request.task.id);
            match uncommitted(worktree) {
                Ok(files) if !files.is_empty() => {
                    self.done(member, uncommitted_work(request, files));
                    continue;
                }
                Ok(_) => {}
                Err(err) => {
                    let outcome =
                        Outcome::error(format!("could not read {}: {err}", request.worktree));
                    self.done(member, outcome);
                    continue;
                }
            }
            match rev(worktree, &format!("refs/heads/{}", request.branch)) {
                Ok(tip) => self.tips[member] = tip,
                Err(err) => self.done(member, Outcome::error(err)),
            }
        }

        // One check at a time in the workspace's folder, as for one delivery.
        let folder = check_folder(lead);
        let folder_turn = turn_of(&format!("check:{}", key(&folder.to_string_lossy())));
        let _folder_turn = folder_turn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut verdicts: Vec<Option<Verdict>> = vec![None; requests.len()];
        let mut moves = 0;
        loop {
            let live = self.live();
            if live.is_empty() {
                return;
            }
            let base = match rev(&self.repo, &format!("refs/heads/{}", lead.target)) {
                Ok(base) => base,
                Err(err) => return self.fail_all(&live, &err),
            };
            let scratch = match Scratch::acquire(&self.repo, &base, folder.clone()) {
                Ok(scratch) => scratch,
                Err(err) => return self.fail_all(&live, &err),
            };
            self.dir = scratch.path.clone();
            self.green = None;

            // 2–3. Merged in turn, checked once, halved while red.
            let (good, end) = self.resolve(base.clone(), live);
            if let Some(err) = self.broken.clone() {
                let live = self.live();
                return self.fail_all(&live, &err);
            }
            if good.is_empty() {
                continue;
            }

            // 4. Each one's own change, read by the reviewer.
            let mut refused = false;
            for (place, link) in good.iter().enumerate() {
                if verdicts[link.member].is_some() {
                    continue;
                }
                match self.review(link, &good[..place], &base, reviewer) {
                    Some(verdict) => verdicts[link.member] = Some(verdict),
                    None => refused = true,
                }
            }
            if refused {
                // What is left goes round again: the trees between the
                // refused one and the end were never checked.
                continue;
            }

            // A member whose worker committed since: not what was checked.
            let mut moved = false;
            for link in &good {
                let request = &requests[link.member];
                let now = rev(
                    Path::new(&request.worktree),
                    &format!("refs/heads/{}", request.branch),
                )
                .unwrap_or_default();
                if now != self.tips[link.member] {
                    moved = true;
                    let detail = format!(
                        "{branch} moved while its delivery was checked in a batch (merged as {}, now at {}): nothing \
of it landed, so that what lands is your latest work. Once that work is finished and committed, call \
delivery_submit again: the new delivery is merged, checked and reviewed before it can land.",
                        short(&self.tips[link.member]),
                        if now.is_empty() { "gone" } else { short(&now) },
                        branch = request.branch,
                    );
                    let outcome = self.outcome(link, Kind::Stale, detail);
                    self.done(link.member, outcome);
                }
            }
            if moved {
                continue;
            }

            // 5. The target moves to the end of the chain: the checked tree.
            for link in &good {
                self.stage(link.member, "landing");
            }
            match land(&self.repo, &lead.target, &base, &end) {
                Landing::Landed(place) => {
                    for link in &good {
                        let request = &requests[link.member];
                        forget_tested(&self.repo, &request.task.id);
                        let detail = format!(
                            "{} {}",
                            landed(request, &link.merge, place.as_deref()),
                            self.note(link.member)
                        );
                        let mut outcome = self.outcome(link, Kind::Landed, detail);
                        outcome.review = verdicts[link.member].clone();
                        outcome.landed_in = place.clone();
                        self.done(link.member, outcome);
                    }
                    return;
                }
                Landing::Refused {
                    place,
                    files,
                    reason,
                } => {
                    // Nothing kept to land as it is: one member alone was
                    // never checked, so landing it later checks it first.
                    for link in &good {
                        let request = &requests[link.member];
                        let detail = format!(
                            "{} {} Since it was checked with others, landing it checks it again first.",
                            in_the_way(request, &place, &files, &reason),
                            self.note(link.member)
                        );
                        let mut outcome = self.outcome(link, Kind::Dirty, detail);
                        outcome.in_checkout = true;
                        outcome.files = files.clone();
                        outcome.review = verdicts[link.member].clone();
                        self.done(link.member, outcome);
                    }
                    return;
                }
                Landing::Moved => {
                    moves += 1;
                    if moves >= LANDING_TRIES {
                        let live = self.live();
                        return self.fail_all(
                            &live,
                            &format!(
                                "{} kept moving while the delivery was tested, {LANDING_TRIES} times in a row. Submit it again once it settles",
                                lead.target
                            ),
                        );
                    }
                    // Tested again on the new head, reviewed again, as one
                    // delivery would be.
                    verdicts.iter_mut().for_each(|verdict| *verdict = None);
                }
                Landing::Failed(err) => {
                    for link in &good {
                        let request = &requests[link.member];
                        let detail = Outcome::error(format!(
                            "landing {} on {} failed: {err}",
                            request.branch, request.target
                        ))
                        .detail;
                        let mut outcome = self.outcome(link, Kind::Error, detail);
                        outcome.review = verdicts[link.member].clone();
                        self.done(link.member, outcome);
                    }
                    return;
                }
            }
        }
    }

    fn fail_all(&mut self, members: &[usize], err: &str) {
        for &member in members {
            self.done(member, Outcome::error(err.to_string()));
        }
    }

    /// `members` merged in turn on `start`; a member that conflicts with
    /// those before it is set aside, one that brings nothing is told so.
    fn chain(&mut self, start: &str, members: &[usize]) -> (Vec<Link>, String) {
        if let Err(err) = Scratch::reset(&self.repo, start, &self.dir) {
            self.broken = Some(err);
            return (vec![], start.to_string());
        }
        let mut links = vec![];
        let mut head = start.to_string();
        for &member in members {
            let request = &self.requests[member];
            self.stage(member, "merging");
            match merge_branch(&self.dir, request, &self.tips[member]) {
                Merged::Head(merge) if merge == head => {
                    self.done(member, Outcome::error(nothing_new(request)));
                }
                Merged::Head(merge) => {
                    links.push(Link {
                        member,
                        parent: head.clone(),
                        merge: merge.clone(),
                    });
                    head = merge;
                }
                Merged::Conflict(_) => {
                    // Alone, after the batch: its conflict, if it still has
                    // one, is the one the target gives it then.
                    let _ = git_run(&self.dir, &["merge", "--abort"], false);
                    if self.settled[member].is_none() {
                        self.settled[member] = Some(Settle::Alone);
                        self.batch(member, None);
                        self.stage(member, "queued");
                    }
                }
                Merged::Failed(err) => {
                    let _ = git_run(&self.dir, &["merge", "--abort"], false);
                    self.done(member, Outcome::error(err));
                }
            }
        }
        (links, head)
    }

    /// The check on `end`, the chain of `links` on `start` (a tree that
    /// passed, or the target), checked out in the check folder.
    fn test(&mut self, start: &str, end: &str, links: &[Link]) -> Tested {
        let changed: Vec<String> = git(&self.dir, &["diff", "--name-only", "-z", start, end])
            .unwrap_or_default()
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect();
        if documentation_only(&changed) {
            return Tested::Green(None, Some(SKIPPED_DOCS.to_string()));
        }
        let lead = &self.requests[0];
        let tree = git(&self.dir, &["rev-parse", &format!("{end}^{{tree}}")])
            .map(|out| out.trim().to_string())
            .unwrap_or_default();
        if !tree.is_empty() && passed_before(lead, &self.repo_key, &tree, &self.line) {
            return Tested::Green(None, Some(SKIPPED_PASSED.to_string()));
        }
        if let Some(check) = self.red.get(&tree) {
            return Tested::Red(check.clone());
        }
        for link in links {
            self.stage(link.member, "checking");
            self.batch(link.member, (links.len() > 1).then_some(links.len()));
        }
        if let Some((main, _, _)) = worktrees(&self.repo)
            .ok()
            .and_then(|list| list.into_iter().find(|(_, _, bare)| !bare))
        {
            copy_local_files(Path::new(&main), &self.dir);
        }
        match run_check(lead, &self.dir, &self.line, self.limits.check) {
            Err(err) => Tested::Broken(err),
            Ok(check) if check.passed => {
                if !tree.is_empty() {
                    remember_passed(lead, &self.repo_key, &tree, &self.line);
                }
                Tested::Green(Some(check), None)
            }
            Ok(check) => {
                if !tree.is_empty() {
                    self.red.insert(tree, check.clone());
                }
                Tested::Red(check)
            }
        }
    }

    /// The members of `members` that pass, merged in turn on `start`, and the
    /// end of their chain: a tree the check passed on (`start` when none
    /// does). Each one that fails is told so with the check of its own merge.
    fn resolve(&mut self, start: String, members: Vec<usize>) -> (Vec<Link>, String) {
        if self.broken.is_some() || members.is_empty() {
            return (vec![], start);
        }
        let (links, end) = self.chain(&start, &members);
        if links.is_empty() || self.broken.is_some() {
            return (vec![], start);
        }
        match self.test(&start, &end, &links) {
            Tested::Broken(err) => {
                self.broken = Some(err);
                (vec![], start)
            }
            Tested::Green(check, skipped) => {
                self.green = Some((check, skipped));
                (links, end)
            }
            Tested::Red(check) if links.len() == 1 => {
                self.blame(&links[0], check);
                (vec![], start)
            }
            Tested::Red(check) => {
                let ids: Vec<usize> = links.iter().map(|link| link.member).collect();
                // The first prefix of the chain that fails, by halves: the
                // first `lo` members pass on `start`, the first `hi` do not.
                let (mut lo, mut hi) = (0, ids.len());
                let mut passed = (vec![], start.clone());
                let mut failed = check;
                while hi - lo > 1 {
                    let mid = (lo + hi) / 2;
                    let (probe, end) = self.chain(&start, &ids[..mid]);
                    if self.broken.is_some() {
                        return (vec![], start);
                    }
                    if probe.len() != mid {
                        // A merge that went otherwise this time (git merges
                        // the same way twice, so it should not): what is
                        // left starts again on `start`.
                        let left: Vec<usize> = ids
                            .iter()
                            .copied()
                            .filter(|&member| self.settled[member].is_none())
                            .collect();
                        return self.resolve(start, left);
                    }
                    match self.test(&start, &end, &probe) {
                        Tested::Broken(err) => {
                            self.broken = Some(err);
                            return (vec![], start);
                        }
                        Tested::Green(check, skipped) => {
                            self.green = Some((check, skipped));
                            lo = mid;
                            passed = (probe, end);
                        }
                        Tested::Red(check) => {
                            hi = mid;
                            failed = check;
                        }
                    }
                }
                // The first `hi - 1` pass and the `hi`th breaks them.
                self.blame(&links[hi - 1], failed);
                // The ones after it, together again on what passed.
                let (mut good, middle) = passed;
                let (rest, end) = self.resolve(middle, ids[hi..].to_vec());
                good.extend(rest);
                (good, end)
            }
        }
    }

    /// `link`'s member is the one the check fails with: `check` is the check
    /// of its own merge, on the members before it that passed.
    fn blame(&mut self, link: &Link, check: Check) {
        let request = &self.requests[link.member];
        let detail = format!(
            "{}\n{} The search through the batch traced the failure to this delivery: the check passes on the \
deliveries merged before it and fails once it is merged.",
            check_failed(request, &self.line, &check, self.limits),
            self.note(link.member)
        );
        let mut outcome = self.outcome(link, Kind::CheckFailed, detail);
        outcome.check = Some(check);
        outcome.check_skipped = None;
        self.done(link.member, outcome);
    }

    /// `link`'s own change read by the reviewer, in the check folder put on
    /// its merge. `before`: the members merged before it. The verdict when it
    /// approves; otherwise the member is settled and `None`.
    fn review(
        &mut self,
        link: &Link,
        before: &[Link],
        base: &str,
        reviewer: &dyn Reviewer,
    ) -> Option<Verdict> {
        let request = &self.requests[link.member];
        self.stage(link.member, "reviewing");
        if let Err(err) = Scratch::reset(&self.repo, &link.merge, &self.dir) {
            let outcome = self.outcome(link, Kind::Error, Outcome::error(err).detail);
            self.done(link.member, outcome);
            return None;
        }
        let (changed, outside) = changes(&self.dir, request, &link.parent, &link.merge);
        let diff = git(
            &self.dir,
            &[
                "diff",
                "--no-color",
                "--no-ext-diff",
                "-M",
                &link.parent,
                &link.merge,
            ],
        )
        .unwrap_or_default();
        let mut prompt = review_prompt(
            request,
            &link.parent,
            &link.merge,
            &changed,
            &outside,
            &diff,
        );
        if !before.is_empty() {
            let branches: Vec<&str> = before
                .iter()
                .map(|other| self.requests[other.member].branch.as_str())
                .collect();
            prompt.push_str(&format!(
                "\nThis delivery is checked together with others. It was merged onto `{}` ({}) with {} merged \
before it, which land with it: the diff above is this delivery's own change, from {} to {}, and this folder holds \
the result with those deliveries in it. Judge this delivery's change only.\n",
                request.target,
                short(base),
                branches.join(", "),
                short(&link.parent),
                short(&link.merge)
            ));
        }
        match reviewer.review(
            &self.dir,
            &request.reviewer_model,
            &prompt,
            request.claude_account.as_deref(),
        ) {
            Ok(verdict) if verdict.passes() => Some(verdict),
            Ok(verdict) => {
                let mut outcome = self.outcome(
                    link,
                    Kind::ChangesRequested,
                    changes_requested(request, &verdict),
                );
                outcome.review = Some(verdict);
                self.done(link.member, outcome);
                None
            }
            Err(err) => {
                let detail = Outcome::error(format!("the review could not run: {err}")).detail;
                let outcome = self.outcome(link, Kind::Error, detail);
                self.done(link.member, outcome);
                None
            }
        }
    }
}
