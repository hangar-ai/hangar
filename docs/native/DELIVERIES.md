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

### Checking it: once, then halves

The check runs once, on the end of the chain.

- Green: every member passed.
- Red, one member: that member fails, with that check's output — exactly the
  check of its own merge.
- Red, several: split in halves `A` and `B`. `A` is checked on main. Then `B`
  is checked on main plus what of `A` passed. When all of `A` passed, the
  tree of `B` on top of it is the tree that just failed, so it is not run
  again: it is red. Each half goes down the same way, to the single members
  that break it.

The rule that keeps main safe: what this returns as *passed* is always a list
of members whose chain, on main, is a tree the check passed on. A member
blamed always failed on its own exact merge. Every check that runs is
remembered by its tree, as before, so a tree that passed never runs twice.

With five deliveries waiting, all green, that is two checks instead of five
(a batch of four, then the fifth alone). One failing in a batch of four:
the batch, its first half, then the half holding the failing one, member by
member — three or four checks for four deliveries, against four.

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
`conflict`, `stale`, `dirty`, `error`); a landed or failed one says it was
checked in a batch, and with which branches. Each member's stage in
`delivery_status` moves as its own did: queued, merging, checking, reviewing,
landing. The batch the delivery is in is known to `delivery_batch_of` in
core.

## Measure

`scripts`-free, by hand: a synthetic repository whose check sleeps 60 s, five
deliveries submitted together, one of them failing its check. See
"Measured" below.

## Measured

(filled in by the measurement run; see the task's comments)
