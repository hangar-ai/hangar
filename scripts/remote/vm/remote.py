#!/usr/bin/env python3
"""hangar-rbuild, the build machine's side: run by scripts/remote/hangar_rbuild.py over SSH on the rented box.

  remote.py hello <idle-min>   state of the box (JSON), and a contact for the watchdog
  remote.py run <payload>      takes a lane, frees disk if short, waits for the PC's "go" (the sources are synced
                               meanwhile), runs the steps. With "affected" in the payload, the line after "go" lists the
                               files that differ from origin/main and affected.plan() picks the crates to check
  remote.py seed-out <lane> [<min-free-gb> <min-free-pct>]
                               copies the lane's dependency artifacts to /root/hb/seed-out, for the PC's mirror
  remote.py seed-pulled <created>
                               the PC has that seed-out: it becomes /root/hb/seed, in place of the older one
  remote.py seed-in            installs the mirror the PC pushed to /root/hb/seed-in as /root/hb/seed
  remote.py disk               free disk, and the size of each lane and seed (for `rbuild status`)

Lanes: /root/hb/lanes/<n>/src (a copy of the caller's worktree) and /root/hb/lanes/<n>/target, one run at a time
each (flock on /root/hb/lanes/<n>.lock). A run prefers the lane its worktree used last, then the one used longest
ago: every lane holds a recent build of the same workspace, so a new worktree, or the scratch worktree of a
delivery check, starts warm. A lane without a target starts from /root/hb/seed (the dependencies of the last
mirror) when it was built by the same rustc.

Disk: before a run writes anything, the box keeps max(<min-free-gb>, <min-free-pct> % of its disk) free. Short of
it, it frees what can be rebuilt, least useful first, and never a lane another run holds: an abandoned seed-out, the
incremental caches and examples of idle lanes (least recently used first), their whole targets, then the run's own
lane. Still short, the run is refused (the PC exits with 75) before a byte is synced.
"""
import base64
import fcntl
import glob
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import time

import affected

HB = "/root/hb"
LANES = f"{HB}/lanes"
STATE = f"{HB}/state"
SEED = f"{HB}/seed"
SEED_OUT = f"{HB}/seed-out"
SEED_IN = f"{HB}/seed-in"
CARGO_HOME = "/root/.cargo"
RUSTUP_HOME = "/root/.rustup"
# Never pruned from a lane's copy of the worktree: target/ holds the kits' wasm build that hangar-kit-host's
# tests make there (tests/tabs.rs), dist-mobile/ is the phone app's stub (an empty folder, as in the CI).
KEEP = {"target", "dist-mobile"}
RUNTIME_DIR = "/tmp/hb-runtime"
GB = 10 ** 9
# What a lane loses first when the disk is short: rustc's incremental caches (the old layout's debug/incremental,
# and the build-dir layout's per-unit ones) and the examples `cargo test` links. The next build redoes them.
LIGHT = ["debug/incremental", "debug/build/*/*/incremental", "debug/examples"]
# A seed-out the PC has not taken this long after it was made is an abandoned pull (the PC promotes it within a
# minute or two of making it).
SEED_OUT_STALE_S = 1800
# A step that fails with less than this free is put down to the disk, not to the code.
DISK_FULL = 2 * GB


def emit(tag, data):
    try:
        sys.stdout.write(f"@@HB-{tag} {json.dumps(data)}\n")
        sys.stdout.flush()
    except OSError:
        pass


def say(text):
    try:
        print(f"[rbuild-box] {text}", file=sys.stderr, flush=True)
    except OSError:
        pass


def touch(path):
    with open(path, "a"):
        pass
    os.utime(path, None)


def contact():
    os.makedirs(STATE, exist_ok=True)
    touch(f"{STATE}/contact")


def read(path, default=""):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return default


def write(path, text):
    with open(path + ".tmp", "w", encoding="utf-8") as f:
        f.write(text)
    os.replace(path + ".tmp", path)


def base_env(color="never"):
    """The CI's environment for cargo, from scratch: nothing of the PC's (no HANGAR_PORT_BASE, no CARGO_TARGET_DIR)."""
    return {
        "HOME": "/root", "USER": "root", "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
        "PATH": f"{CARGO_HOME}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        "CARGO_HOME": CARGO_HOME, "RUSTUP_HOME": RUSTUP_HOME,
        "CARGO_TERM_COLOR": color, "TERM": "xterm-256color" if color == "always" else "dumb",
        # ci.yml: file and line in a backtrace without the full debug info (less to compile, link and keep).
        "CARGO_PROFILE_DEV_DEBUG": "line-tables-only",
        # ci.yml, Tests step: the views' tests draw with iced_test's software renderer.
        "ICED_TEST_BACKEND": "tiny-skia",
        "CI": "true",
        # set on a GitHub runner; without it, GTK and D-Bus clients in the tests print an error each
        "XDG_RUNTIME_DIR": RUNTIME_DIR,
    }


def rustc_version():
    r = subprocess.run([f"{CARGO_HOME}/bin/rustc", "-vV"], capture_output=True, text=True, env=base_env())
    return r.stdout.strip()


def seed_key(rustc, src):
    lock = read(os.path.join(src, "Cargo.lock"))
    return hashlib.sha256((rustc + "\0" + lock).encode()).hexdigest()[:16]


def lane_meta(i):
    try:
        return json.loads(read(f"{LANES}/{i}.json", "{}"))
    except ValueError:
        return {}


def try_lock(i):
    f = open(f"{LANES}/{i}.lock", "a")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return f
    except BlockingIOError:
        f.close()
        return None


def acquire(count, key):
    os.makedirs(LANES, exist_ok=True)
    metas = [(i, lane_meta(i)) for i in range(count)]
    # its own lane first, then lanes already built (longest unused first), then empty ones
    order = sorted(metas, key=lambda m: (m[1].get("key") != key, not m[1].get("used"), m[1].get("used", 0)))
    waited = None
    while True:
        for i, _ in order:
            f = try_lock(i)
            if f:
                if waited:
                    say(f"lane {i} free after {time.time() - waited:.0f} s")
                return i, f
        if waited is None:
            waited = time.time()
            say(f"all {count} lanes are busy: waiting for one")
        contact()
        time.sleep(2)


def busy_lanes(count, mine):
    busy = 0
    for i in range(count):
        if i == mine:
            continue
        f = try_lock(i)
        if f:
            f.close()
        else:
            busy += 1
    return busy


def prune(src, listfile):
    """Removes what the caller's worktree no longer has (a deleted or renamed file), then the empty folders: a
    leftover crates/<x>/ without its Cargo.toml would break the workspace's `crates/*` glob."""
    keep = {x for x in open(listfile, "rb").read().decode("utf-8", "surrogateescape").split("\0") if x}
    removed = 0
    for root, dirs, files in os.walk(src):
        rel_root = os.path.relpath(root, src)
        if rel_root == ".":
            dirs[:] = [d for d in dirs if d not in KEEP]
        for name in files + [d for d in dirs if os.path.islink(os.path.join(root, d))]:
            rel = name if rel_root == "." else os.path.join(rel_root, name)
            if rel not in keep:
                os.unlink(os.path.join(root, name))
                removed += 1
    for root, dirs, files in os.walk(src, topdown=False):
        rel_root = os.path.relpath(root, src)
        if rel_root != "." and rel_root.split(os.sep, 1)[0] not in KEEP and not os.listdir(root):
            os.rmdir(root)
    return removed


class SeedLock:
    """Seed folders are rewritten by seed-out, seed-pulled and seed-in (exclusive) and copied into empty lanes
    (shared). With blocking=False, `held` says whether it was had at once."""

    def __init__(self, exclusive, blocking=True):
        self.mode = (fcntl.LOCK_EX if exclusive else fcntl.LOCK_SH) | (0 if blocking else fcntl.LOCK_NB)

    def __enter__(self):
        os.makedirs(STATE, exist_ok=True)
        self.f = open(f"{STATE}/seed.lock", "a")
        try:
            fcntl.flock(self.f, self.mode)
            self.held = True
        except BlockingIOError:
            self.held = False
        return self

    def __exit__(self, *exc):
        self.f.close()
        return False


# ------------------------------------------------------------------------------------------------ disk
def disk():
    """Free bytes (as a run sees them) and the size of the box's disk."""
    st = os.statvfs(HB)
    return st.f_bavail * st.f_frsize, st.f_blocks * st.f_frsize


def size(path, exclude=()):
    """Bytes on disk under path (a file hard-linked twice in it counted once), 0 when it is not there."""
    if not os.path.lexists(path):
        return 0
    r = subprocess.run(["du", "-sB1", *(f"--exclude={x}" for x in exclude), path], capture_output=True, text=True, check=False)
    try:
        return int(r.stdout.split()[0])
    except (IndexError, ValueError):
        return 0


def light_size(target):
    """What pruning a lane's incremental caches and examples would free. Not their own du: cargo hard-links what it
    puts in debug/examples to debug/build, and those blocks stay."""
    return max(0, size(target) - size(target, exclude=("incremental", "debug/examples")))


def lane_dirs():
    """Every lane on the box, those beyond the PC's HANGAR_RBUILD_LANES included (they take disk all the same)."""
    try:
        return sorted(int(n) for n in os.listdir(LANES) if n.isdigit() and os.path.isdir(f"{LANES}/{n}"))
    except OSError:
        return []


def prune_lane(i, whole):
    """Lane i's incremental caches and examples, or its whole target (the next run there starts from the seed).
    The caller holds the lane's lock. Bytes freed."""
    target = f"{LANES}/{i}/target"
    paths = [target] if whole else [p for g in LIGHT for p in glob.glob(f"{target}/{g}")]
    if not paths:
        return 0
    before = size(target)  # the whole target before and after: its hard links make each folder's own du wrong
    for path in paths:
        shutil.rmtree(path, ignore_errors=True)
    return max(0, before - size(target))


def seed_out_abandoned():
    """seed-out made over SEED_OUT_STALE_S ago and never taken by the PC (a pull that failed or was cut)."""
    if not os.path.isdir(SEED_OUT):
        return False
    try:
        made = json.loads(read(f"{SEED_OUT}/hb-seed.json", "{}") or "{}").get("created") or os.path.getmtime(SEED_OUT)
    except (ValueError, OSError):
        made = 0
    return time.time() - made > SEED_OUT_STALE_S


def ensure_space(own, min_gb, pct, own_whole=True):
    """At least max(min_gb, pct % of the disk) free before a run (or a seed-out) writes anything. Short of it, frees
    what can be rebuilt, least useful first: an abandoned seed-out, the incremental caches and examples of the idle
    lanes (their lock free; least recently used first), their whole targets, then the caller's own lane `own`
    (whose lock it holds): its caches, then (own_whole) its target. A lane another run holds is never touched.
    One guard at a time (state/disk.lock). Returns what it found and did; `ok` False when still short."""
    os.makedirs(STATE, exist_ok=True)
    with open(f"{STATE}/disk.lock", "a") as guard_lock:
        fcntl.flock(guard_lock, fcntl.LOCK_EX)
        free, total = disk()
        need = max(min_gb * GB, total * pct / 100)
        rep = {"free": free, "total": total, "need": round(need), "pruned": [], "busy": []}
        if free < need and seed_out_abandoned():
            with SeedLock(exclusive=True, blocking=False) as lk:
                if lk.held:
                    freed = size(SEED_OUT)
                    shutil.rmtree(SEED_OUT, ignore_errors=True)
                    rep["pruned"].append({"what": "seed-out", "bytes": freed})
        others = sorted((i for i in lane_dirs() if i != own), key=lambda i: (lane_meta(i).get("used") or 0, i))
        ladder = [(i, False) for i in others] + [(i, True) for i in others] + [(own, False)] + ([(own, True)] if own_whole else [])
        for i, whole in ladder:
            if disk()[0] >= need:
                break
            if i == own:
                freed = prune_lane(i, whole)
            else:
                f = try_lock(i)
                if f is None:
                    if i not in rep["busy"]:
                        rep["busy"].append(i)
                    continue
                try:
                    freed = prune_lane(i, whole)
                finally:
                    f.close()
            if freed:
                rep["pruned"].append({"lane": i, "what": "target" if whole else "incremental", "bytes": freed})
        rep["free_after"] = disk()[0]
        rep["ok"] = rep["free_after"] >= need
    return rep


def lane_target(i, rustc):
    """The lane's target dir. An empty one starts from the newest seed built by this rustc: the PC's mirror pushed
    at the box's creation (seed/), or what a lane of this box built (seed-out/, made for the mirror)."""
    target = f"{LANES}/{i}/target"
    tag = f"{LANES}/{i}/rustc.txt"
    if os.path.isdir(target) and read(tag) != rustc:
        say(f"lane {i}: another rustc than its last build, target cleared")
        shutil.rmtree(target, ignore_errors=True)
    if not os.path.isdir(target):
        with SeedLock(exclusive=False):
            seeds = []
            for folder in (SEED, SEED_OUT):
                try:
                    info = json.loads(read(f"{folder}/hb-seed.json", "{}") or "{}")
                except ValueError:
                    info = {}
                if info.get("rustc") == rustc and os.path.isdir(f"{folder}/target"):
                    seeds.append((info.get("created", 0), folder, info))
            if seeds:
                _, folder, info = max(seeds)
                t0 = time.time()
                cp = subprocess.run(["cp", "-a", f"{folder}/target", target], capture_output=True, text=True, check=False)
                if cp.returncode == 0:
                    say(f"lane {i}: dependencies copied from {os.path.basename(folder)} (key {info.get('key')}) in {time.time() - t0:.0f} s")
                else:  # a disk filled by other lanes since the guard: cargo meets it too, and the run says so
                    shutil.rmtree(target, ignore_errors=True)
                    say(f"lane {i}: the seed could not be copied ({cp.stderr.strip()[-160:]}): cold build")
            else:
                say(f"lane {i}: no seed built by {rustc.splitlines()[0] if rustc else 'this rustc'}: cold build")
        os.makedirs(target, exist_ok=True)
    write(tag, rustc)
    return target


CHILD = {"proc": None}


def stop_child(signum, _frame):
    p = CHILD["proc"]
    if p and p.poll() is None:
        try:
            os.killpg(p.pid, signal.SIGTERM)
            for _ in range(50):
                if p.poll() is not None:
                    break
                time.sleep(0.2)
            else:
                os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    os._exit(128 + signum)


def run(payload):
    p = json.loads(base64.b64decode(payload))
    contact()
    if os.path.exists(f"{STATE}/retired"):
        emit("RETIRED", {"why": read(f"{STATE}/watchdog.log").strip().splitlines()[-1:]})
        return 75
    for s in (signal.SIGHUP, signal.SIGTERM, signal.SIGINT):
        signal.signal(s, stop_child)
    t0 = time.time()
    i, _lock = acquire(int(p["lanes"]), p["key"])
    wait_s = round(time.time() - t0, 1)
    if "min_free_gb" in p:
        space = ensure_space(i, float(p["min_free_gb"]), float(p.get("min_free_pct") or 0))
        emit("DISK", {**space, "lane": i})
        if not space["ok"]:
            return 75
    src = f"{LANES}/{i}/src"
    os.makedirs(src, exist_ok=True)
    emit("LANE", {"lane": i, "src": src, "list": f"{LANES}/{i}.files", "wait_s": wait_s})
    if sys.stdin.readline().strip() != "go":
        return 0
    changed = None
    if p.get("affected"):
        try:
            changed = json.loads(sys.stdin.readline() or "null")
        except ValueError:
            changed = None
        if not isinstance(changed, dict):
            changed = {"error": "the PC's list of changed files did not arrive"}
    t1 = time.time()
    removed = prune(src, f"{LANES}/{i}.files")
    os.makedirs(f"{src}/dist-mobile", exist_ok=True)
    rustc = rustc_version()
    target = lane_target(i, rustc)
    prep_s = round(time.time() - t1, 1)
    # One runtime folder per lane: runs share the machine's user, and tests that
    # bind sockets under XDG_RUNTIME_DIR by fixed names (the Codex pane servers)
    # would take each other's in two lanes at once.
    runtime = f"{RUNTIME_DIR}/lane-{i}"
    os.makedirs(runtime, mode=0o700, exist_ok=True)
    env = base_env(p.get("color", "never"))
    env["XDG_RUNTIME_DIR"] = runtime
    env["CARGO_TARGET_DIR"] = target
    if p.get("incremental") in ("0", "1"):
        env["CARGO_INCREMENTAL"] = p["incremental"]
    to_run, planned = p["steps"], None
    if changed is not None:
        # The run is never refused for what planning could not read: it falls back to the full check.
        cargo_meta = None
        if not changed.get("error"):
            try:
                cargo_meta = metadata(src, env)
            except (OSError, ValueError, subprocess.SubprocessError) as e:
                say(f"cargo metadata: {e}")
        try:
            planned = affected.plan(p["steps"], changed, cargo_meta, src)
        except Exception as e:  # a planner's bug costs time (the full check), never a verdict on the wrong crates
            why = f"planning failed ({e!r})"
            planned = {"mode": "full", "steps": p["steps"], "packages": [], "total": 0, "why": why,
                       "lines": [f"full check: {why}"]}
        for line in planned["lines"]:
            say(line)
        emit("PLAN", {k: planned[k] for k in ("mode", "packages", "total", "why")})
        to_run = planned["steps"]
    steps, code = [], 0
    for step in to_run:
        say(f"{step['name']}: {' '.join(step['argv'])}")
        contact()
        s0 = time.time()
        e = dict(env)
        e.update(step.get("env", {}))
        for name in step.get("unset", []):
            e.pop(name, None)
        proc = subprocess.Popen(step["argv"], cwd=src, env=e, stdin=subprocess.DEVNULL, start_new_session=True)
        CHILD["proc"] = proc
        rc = proc.wait()
        CHILD["proc"] = None
        steps.append({"name": step["name"], "code": rc, "s": round(time.time() - s0, 1)})
        if rc != 0:
            code = code or rc
            if not p.get("keep_going"):
                break
    contact()
    meta = {"key": p["key"], "worktree": p.get("worktree", ""), "used": time.time(), "kind": p.get("kind", ""),
            # only a whole-workspace test run leaves every dependency built: what a new lane's seed is taken from
            "tested": (planned is None or planned["mode"] == "full") and any(s["name"] == "test" for s in steps),
            "seed_key": seed_key(rustc, src),
            "rustc": rustc.splitlines()[0] if rustc else ""}
    free = disk()[0]
    try:
        write(f"{LANES}/{i}.json", json.dumps(meta))
    except OSError:
        pass  # a full disk: the lane is still the least recently used one, which is what its next pruning wants
    emit("EXIT", {"code": code, "lane": i, "removed": removed, "prep_s": prep_s, "steps": steps, "tested": meta["tested"],
                  "busy": busy_lanes(int(p["lanes"]), i), "seed_key": meta["seed_key"], "rustc": meta["rustc"],
                  "free": free, "disk_full": code != 0 and free < DISK_FULL})
    return code


def metadata(src, env):
    """The workspace's crates (`cargo metadata --no-deps`), as cargo prints them."""
    r = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"], cwd=src, env=env,
                       capture_output=True, text=True, timeout=120)
    if r.returncode != 0:
        raise ValueError(r.stderr.strip()[-400:] or f"exit {r.returncode}")
    return json.loads(r.stdout)


def members(src, env):
    try:
        data = metadata(src, env)
    except (ValueError, OSError, subprocess.SubprocessError) as e:
        raise SystemExit(f"cargo metadata: {e}")
    pkgs = {pkg["name"] for pkg in data["packages"]}
    targets = {t["name"].replace("-", "_") for pkg in data["packages"] for t in pkg["targets"]}
    return pkgs, targets


def seed_out(lane, min_gb=20, pct=15):
    """The lane's dependency artifacts, without the workspace's own crates (they change with every commit) nor the
    incremental caches: what swatinem/rust-cache keeps in the CI. rsync --delete keeps seed-out in step. The disk
    guard runs first, as before a build: the copy is several GB."""
    i = int(lane)
    contact()
    f = try_lock(i)
    while f is None:
        time.sleep(2)
        contact()
        f = try_lock(i)
    space = ensure_space(i, float(min_gb), float(pct), own_whole=False)
    if not space["ok"]:
        f.close()
        emit("DISK", space)
        raise SystemExit(f"seed-out: the disk is short ({space['free_after'] / GB:.1f} GB free, {space['need'] / GB:.1f} GB kept) "
                         f"and lanes {space['busy']} are busy: no copy made")
    src, target = f"{LANES}/{i}/src", f"{LANES}/{i}/target"
    env = base_env()
    pkgs, targets = members(src, env)
    # cargo's build-dir layout since Rust 1.100: debug/build/<package>/<hash>/{fingerprint,out,run} holds a unit's
    # artifacts, fingerprint and build-script run; the older one (deps/, .fingerprint/, build/<package>-<hash>/)
    # is filtered the same way in case a toolchain still writes it.
    rules = ["- /debug/incremental/", "- /debug/examples/"]
    rules += [f"- /debug/build/{n}/" for n in sorted(pkgs)] + [f"- /debug/build/{n}-*" for n in sorted(pkgs)]
    rules += [f"- /debug/.fingerprint/{n}-*" for n in sorted(pkgs)]
    rules += [f"- /debug/deps/{t}-*" for t in sorted(targets)] + [f"- /debug/deps/lib{t}-*" for t in sorted(targets)]
    rules += ["- /debug/build/*/*/incremental/", "+ /debug/", "+ /debug/deps/***", "+ /debug/build/***",
              "+ /debug/.fingerprint/***", "+ /.rustc_info.json", "+ /CACHEDIR.TAG", "- *"]
    with open(f"{STATE}/seed-rules", "w") as rf:
        rf.write("\n".join(rules) + "\n")
    t0 = time.time()
    rustc = rustc_version()
    info = {"key": seed_key(rustc, src), "rustc": rustc, "lane_target": target, "created": time.time(),
            "worktree": lane_meta(i).get("worktree", "")}
    with SeedLock(exclusive=True):
        os.makedirs(f"{SEED_OUT}/target", exist_ok=True)
        if os.path.exists(f"{SEED_OUT}/hb-seed.json"):
            os.remove(f"{SEED_OUT}/hb-seed.json")
        subprocess.run(["rsync", "-a", "--delete", "--delete-excluded", f"--filter=merge {STATE}/seed-rules",
                        target + "/", f"{SEED_OUT}/target/"], check=True)
        write(f"{SEED_OUT}/hb-seed.json", json.dumps(info))
    f.close()
    du = subprocess.run(["du", "-sb", f"{SEED_OUT}/target", f"{CARGO_HOME}/registry/cache", f"{CARGO_HOME}/registry/index"],
                        capture_output=True, text=True).stdout.split()
    info.update(copy_s=round(time.time() - t0, 1), target_bytes=int(du[0]) if du else 0,
                registry_bytes=sum(int(x) for x in du[2::2]) if du else 0, disk=space)
    emit("SEED", info)
    return 0


def seed_pulled(created):
    """The PC's mirror now holds seed-out: it becomes /root/hb/seed, what this box's empty lanes start from, in place
    of the older seed (the mirror pushed at the creation, or a previous seed-out). One copy on the disk, not two.
    Left alone if seed-out was remade since (another pull)."""
    with SeedLock(exclusive=True):
        try:
            info = json.loads(read(f"{SEED_OUT}/hb-seed.json", "{}") or "{}")
        except ValueError:
            info = {}
        if not info or abs(float(info.get("created", 0)) - float(created)) > 0.01:
            emit("PULLED", {"promoted": False, "why": "seed-out is not the one pulled"})
            return 0
        freed = size(SEED)
        shutil.rmtree(SEED, ignore_errors=True)
        os.rename(SEED_OUT, SEED)
    free, total = disk()
    emit("PULLED", {"promoted": True, "freed": freed, "seed": size(SEED), "free": free, "total": total})
    return 0


def disk_report():
    """For `rbuild status`: free disk, and what each lane and seed holds. Not a contact for the watchdog."""
    free, total = disk()
    lanes = {}
    for i in lane_dirs():
        f = try_lock(i)
        if f:
            f.close()
        meta = lane_meta(i)
        target = f"{LANES}/{i}/target"
        lanes[i] = {"bytes": size(target), "light": light_size(target), "busy": f is None, "used": meta.get("used"),
                    "worktree": meta.get("worktree", "")}
    emit("DISK", {"free": free, "total": total, "lanes": lanes, "seed": size(SEED), "seed_out": size(SEED_OUT),
                  "registry": size(f"{CARGO_HOME}/registry")})
    return 0


def seed_in():
    """The PC pushed its mirror to /root/hb/seed-in (target/ and hb-seed.json; the registry went straight to
    /root/.cargo/registry): it becomes /root/hb/seed, what empty lanes start from."""
    if not os.path.isfile(f"{SEED_IN}/hb-seed.json"):
        raise SystemExit("seed-in: no hb-seed.json")
    with SeedLock(exclusive=True):
        shutil.rmtree(SEED, ignore_errors=True)
        os.rename(SEED_IN, SEED)
    info = json.loads(read(f"{SEED}/hb-seed.json"))
    emit("SEEDED", {"key": info.get("key"), "same_rustc": info.get("rustc") == rustc_version()})
    return 0


def hello(idle_min):
    os.makedirs(STATE, exist_ok=True)
    write(f"{STATE}/idle-min", str(int(idle_min)))
    contact()
    st = os.statvfs("/root")
    mem = 0
    for line in open("/proc/meminfo"):
        if line.startswith("MemTotal:"):
            mem = int(line.split()[1]) // 1048576
    lanes = {}
    for name in sorted(os.listdir(LANES)) if os.path.isdir(LANES) else []:
        if name.endswith(".json"):
            lanes[name[:-5]] = lane_meta(name[:-5])
    emit("HELLO", {"retired": os.path.exists(f"{STATE}/retired"), "nproc": os.cpu_count(), "mem_gb": mem,
                   "free_gb": round(st.f_bavail * st.f_frsize / 2 ** 30, 1), "rustc": rustc_version().splitlines()[:1],
                   "lanes": lanes, "seed": json.loads(read(f"{SEED}/hb-seed.json", "{}") or "{}").get("key"),
                   "watchdog": subprocess.run(["pgrep", "-f", "/root/hb/bin/watchdog.sh"], capture_output=True).returncode == 0})
    return 0


def main():
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd == "run":
        return run(args[0])
    if cmd == "hello":
        return hello(args[0] if args else 60)
    if cmd == "seed-out":
        return seed_out(*args[:3])
    if cmd == "seed-pulled":
        return seed_pulled(args[0])
    if cmd == "seed-in":
        return seed_in()
    if cmd == "disk":
        return disk_report()
    raise SystemExit(f"remote.py: unknown command {cmd}")


if __name__ == "__main__":
    sys.exit(main())
