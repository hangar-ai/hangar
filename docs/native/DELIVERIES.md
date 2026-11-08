# Deliveries: the merge queue

A worker calls `delivery_submit`; Hangar merges its branch onto the
workspace's branch (the *target*, "main" below) in a checkout of its own,
runs the workspace's check there, has a reviewer read the change and lands
it. All of it is `crates/hangar-core/src/delivery.rs`.

Until 2.2 the deliveries of a repository went through that one at a time:
five waiting deliveries were five full checks in a row, ten to twenty minutes
each on a big project. From 2.3 the deliveries that wait together are
**checked together, in a batch**: merged one after the other on main, checked
once, and split in halves only when that check fails. Main stays exactly as
safe: it only ever moves to a tree the check passed on.

## One delivery, as before

1. Only committed work is delivered (uncommitted files: `dirty`).
2. The branch is merged onto main as it is now, in the workspace's check
   checkout (`<data_local>/hangar/c/<8 hex>`), a git worktree made once and
   reset to each merge. Ignored files stay (dependencies), and the build
   output is outside it (`build_cache::check_env_for`): a check is
   incremental. A conflict: `conflict`, with the files.
3. The check runs on the merge, unless nothing but documentation changed or
   the very same tree passed it before (`delivery-passed.json`). Red:
   `checkFailed`, with the end of its output.
4. A reviewer that did not write it reads the diff (`changesRequested` when
   it refuses).
5. Main is fast-forwarded to the merge (`landed`), or, with automatic landing
   off, the merge is kept for the person to land (`ready`).

The queue is per repository (its git common dir): one delivery holds the
repository's *turn* from step 2 to step 5, the others wait for it.

## A batch

When a delivery gets the turn, it looks at who waits with it. The deliveries
that can share a check are taken together, in the order they were submitted,
up to the batch size (4):

- they land by themselves (automatic landing on) and are not a landing of
  what waited for the person (`reuse`): a delivery kept for the person must
  land as it was tested, alone, and a batch's merge holds other deliveries'
  work the person never chose to land;
- the workspace has a check: without one there is nothing to share;
- they have the same target, check command, check folder, shell and build
  environment, so one check run says the same about all of them.

Fewer than two such deliveries: the delivery goes alone, the path above,
unchanged. A delivery submitted while a batch runs waits for the next one.

### Building it

From main's head `B`, in the check checkout, each member in turn is merged on
top of the one before: `B → M1 → M2 → M3`. A member's *own* change is
`M(i-1) → Mi`: exactly what it would have been, merged on a main where the
members before it had landed, as the sequential queue would have done.

- Uncommitted work in a member's worktree: `dirty` for it, as before.
- A member that conflicts on `B` itself: `conflict`, as before.
- A member that merges cleanly on `B` but conflicts with a member before it:
  **set aside**. It leaves the batch and goes alone once the batch is over,
  on the main the batch left: its conflict (or its clean merge, when what it
  clashed with did not land) is the one the sequential queue would have given.
- A member whose merge changes nothing (all of it is on main or in the
  members before it): the usual "nothing new" error.

### Checking it: once, then by halves

The check runs once, on the end of the chain.

- Green: every member passed.
- Red, one member: that member fails, with that check's output — exactly the
  check of its own merge.
- Red, several: Hangar looks for the first member that breaks it, by halves
  over the chain's prefixes. The whole chain is red; the first half (main +
  the first half of the members) is checked: green, the culprit is in the
  second half and the next probe is three quarters; red, it is in the first
  half and the next probe is a quarter; and so on down to `k` members that
  pass and `k + 1` that fail. Member `k + 1` gets `checkFailed`, with the
  check of its own merge (main + the `k` before it). The members after it are
  merged again, together, on the `k` that passed, and that chain goes the
  same way: checked once, searched only if red.

This is the brief's "first half on main, second half on main + first half"
when the first half is green. When the first half is red, the members after
the culprit are checked together again rather than half by half: one check
instead of two or more.

The rule that keeps main safe: what this returns as *passed* is always a list
of members whose chain, on main, is a tree the check passed on. A member
blamed always failed on its own exact merge. Every tree the check failed on
is remembered for the batch, and every tree it passed on is remembered as
before (`delivery-passed.json`), so no tree is checked twice: the tree of the
second half on a green first half is the one that just failed, and is not run
again.

The cost, for a batch of `n` with one member failing: the batch, about
`log2(n)` probes, and the members after the culprit together — four checks
for a batch of four, four for five. All green: one check for the batch.

A check that cannot run (its shell is missing, Hangar quits): `error` for
every member, nothing merged, as before.

### Reviewing it

Once the check passed, each member is reviewed in turn, on its own diff
(`M(i-1) → Mi`), with the check checkout on its own merge `Mi` so that the
reviewer reads the files as they were with that delivery. The prompt says it
was checked with the deliveries before it. Reviews run one after the other:
in parallel they would each need a checkout of their own.

### Landing it: the whole checked chain, or a new check

- Every member approved: main is fast-forwarded to the end of the chain, the
  tree the check passed on. Each member gets `landed`, with its own merge and
  its own base.
- A member refused: it gets `changesRequested`, as before. The others do
  **not** land up to it: the trees in between were never checked. The
  members still in go round again — merged on main as it is then, checked
  (skipped when that very tree passed already, for instance in the halving),
  and landed. Their reviews stand: what each one changes is the same, and
  the check judges how they fit together.
- A member whose branch moved while the batch ran (the worker committed
  again): what was checked is not its latest work. It gets `stale`, nothing
  of it lands, and the others go round again without it.
- Main moved meanwhile (the person committed): the batch starts again on the
  new head, three times at most, as the single delivery does.
- The person's checkout of main has local changes in the way: every member
  gets `dirty` in the checkout and waits for the person, as before. Unlike a
  single delivery, no tested merge is kept: landing one member alone from
  the batch would land a tree that was never checked, so landing it later
  merges and checks it again first.

Every round removes at least one member or ends the batch, so it ends.

### What workers and the orchestrator see

Each member's `delivery_submit` gets its own outcome, with the same kinds and
the same detail as before (`landed`, `checkFailed`, `changesRequested`,
`conflict`, `stale`, `dirty`, `error`): the board, the worker's message and
the orchestrator's event follow from it exactly as for a delivery on its own.
A landed or failed one adds that it was checked in a batch, and with which
branches. Each member's stage in `delivery_status` moves as its own did
(queued, merging, checking, reviewing, landing: Hangar Mobile's stages are
unchanged), and while it is checked with others `delivery_status` adds
`batch`: how many deliveries the check it is in covers (`Running::batch`).

## The setting

"Deliveries checked together", in the workspace's orchestration settings
(`OrchestrationSettings::batch_size`, `batchSize` in the file, written only
when set): 1 (one at a time, as before 2.3), 2, 3, 4 (the default,
`delivery::BATCH_SIZE`), 6 or 8; a file may hold up to 16
(`delivery::MAX_BATCH_SIZE`). It reaches the queue in `Delivery::batch_size`;
the size of the delivery that takes the turn is the one used.

## Not done

- Reviews in parallel: each would need a checkout of its own.
- Filling the re-check after a culprit with deliveries still waiting beyond
  the batch size: the fifth of five waiting deliveries goes in the next
  batch even when the first one had room left after a failure.

## Measure

`delivery::tests::batches::measure_a_queue_of_five_with_one_failing`, run by
hand (`cargo test -p hangar-core --lib measure_a_queue -- --ignored
--nocapture`, `HANGAR_MEASURE_CHECK_SECS` for the check's length): throwaway
repositories, five deliveries waiting together, a check that sleeps 60 s and
fails when `bad.rs` is there, the third delivery adding it — then the same
with all five green. Once one at a time (batch size 1), then with batches of
4 and 5; it asserts that every delivery's outcome and the files that end on
main are the same as one at a time. The reviewer is a stand-in that approves:
the review's minutes are the same either way, one per delivery.

## Measured

2026-11-08, Linux, a check of 60 s, five deliveries waiting, the review a
stand-in (add one real review per delivery to every line):

| | one at a time | batches of 4 (default) | batches of 5 |
|---|---|---|---|
| one failing (the third) | 314.8 s, 5 checks | 256.4 s, 4 checks | 257.1 s, 4 checks |
| all green | 315.4 s, 5 checks | 134.2 s, 2 checks | 73.5 s, 1 check |

Every delivery had the same outcome as one at a time (four landed and the
third `checkFailed`; all five landed), and main held the same files. With a
failure the gain is a check or so: finding the culprit costs about
`log2(n)` probes, and what follows it is checked once more. With every
delivery green, five checks become one per batch. The deliveries of a batch
are taken in the order they joined the queue, which the threads of the
measure do not fix: with one failing, the batch of 4 can take 4 or 5 checks
depending on where the failing one falls in it (4 here; 5 in a shorter run
of the same measure with a 1 s check).
