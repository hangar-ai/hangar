"""hangar-rbuild, the build machine's side: which crates a change reaches (`rbuild check --affected`).

Pure functions, no I/O beyond reading the workspace's sources: remote.py runs `cargo metadata` and calls plan(). The
PC sends the files that differ from origin/main's merge-base (git is only there: the lanes have no .git).

A changed file is mapped to the workspace crates that
  - own it: the file is under the crate's folder (every crate whose folder contains it, for a crate nested in another);
  - read it from outside their own folder: an `include_str!`/`include_bytes!`/`include!`/`#[path]` that leaves the crate,
    or a test's `env!("CARGO_MANIFEST_DIR")` joined with a `../…` path (found by scanning the crates' .rs files).
The crates that depend on those, directly or not (any kind of dependency: normal, dev, build), come with them, and so do
those whose tests run cargo on another crate (`"-p", "<crate>"`, as hangar-kit-host's tests do on the example kits).

What it never takes a chance on, and answers with the full check: a manifest, the lockfile, a toolchain file, rustfmt's or
clippy's settings, `.cargo/`, a build script or the remote build's own scripts changed (they change how everything
builds), a file no crate reads and that is not plainly documentation, a change that touches every crate, no change at
all, or anything this module cannot read.

Python 3.10 (the box's ubuntu 22.04): no 3.11+ syntax here.
"""
import os
import posixpath
import re

# Their change can alter how any crate builds or is judged: the full check.
FULL_NAMES = {
    "Cargo.toml": "manifest",
    "Cargo.lock": "lockfile",
    "build.rs": "build script",
    "rust-toolchain": "toolchain",
    "rust-toolchain.toml": "toolchain",
    "rustfmt.toml": "rustfmt settings",
    ".rustfmt.toml": "rustfmt settings",
    "clippy.toml": "clippy settings",
    ".clippy.toml": "clippy settings",
}
FULL_DIRS = {".cargo": "cargo configuration", "scripts/remote": "remote build script"}
# What no check of the Rust workspace reads (a crate that does is found by the scan, which comes first): the docs, the
# site, the phone app's sources (the CI builds the app with an empty dist-mobile/), the CI's workflows, the other
# scripts, the packaging files, and a root file that is not Rust.
IGNORED_DIRS = {"docs", "site", "mobile", ".github", ".vscode", "nix", "scripts"}
IGNORED_ROOT_FILES = {"LICENSE", ".gitignore", ".dockerignore", "Dockerfile", "flake.nix", "flake.lock", "package.json",
                      "pnpm-lock.yaml", "pnpm-workspace.yaml", "sws.toml"}
SKIP_DIRS = {"target", ".git", "node_modules", "dist-mobile"}

INCLUDE = re.compile(r'\binclude(?:_str|_bytes)?!\s*\(\s*"([^"\\\n]+)"')
PATH_ATTR = re.compile(r'#\[\s*path\s*=\s*"([^"\\\n]+)"\s*\]')
MANIFEST_DIR = re.compile(r"CARGO_MANIFEST_DIR")
# A path that climbs out of the crate and goes on: `"../../assets/x.glb"`, or `"/../../assets/x.png"` after a concat!. A bare
# `".."` is a step of a chain of joins that cannot be followed.
DOTS = re.compile(r'"/?((?:\.\./)+[^"\\\n]+)"')
CARGO_P = re.compile(r'"-p"\s*,\s*"([A-Za-z0-9_-]+)"')
WINDOW = 200


def load(meta):
    """The workspace's crates from `cargo metadata --no-deps --format-version 1`: ({name: {"dir": the folder relative
    to the workspace root ("" for a root package), "deps": names of the workspace crates it depends on, any kind}},
    the build scripts' paths)."""
    root = meta["workspace_root"]
    pkgs, build_scripts = {}, set()
    for p in meta["packages"]:
        rel = os.path.relpath(os.path.dirname(p["manifest_path"]), root).replace(os.sep, "/")
        pkgs[p["name"]] = {"dir": "" if rel == "." else rel, "deps": set()}
    for p in meta["packages"]:
        for d in p.get("dependencies") or []:
            if d["name"] in pkgs and d["name"] != p["name"] and d.get("path"):
                pkgs[p["name"]]["deps"].add(d["name"])
        for t in p.get("targets") or []:
            if "custom-build" in (t.get("kind") or []):
                build_scripts.add(os.path.relpath(t["src_path"], root).replace(os.sep, "/"))
    return pkgs, build_scripts


def inside(path, folder):
    return path == folder or path.startswith(folder + "/") or folder == ""


def owners(path, pkgs):
    """Every crate whose folder holds the file (the nested one and the one around it)."""
    return {n for n, p in pkgs.items() if inside(path, p["dir"])}


def innermost(path, pkgs):
    best = None
    for n, p in pkgs.items():
        if inside(path, p["dir"]) and (best is None or len(p["dir"]) > len(pkgs[best]["dir"])):
            best = n
    return best


def scan(root, pkgs):
    """What the crates' sources read outside their own folder, and the crates they run cargo on.
    -> (refs: [(path relative to the root, crate that reads it)], edges: {(crate, crate it runs cargo on)})"""
    refs, edges, seen = [], set(), set()
    for name, pkg in sorted(pkgs.items()):
        for folder, dirs, files in os.walk(os.path.join(root, pkg["dir"])):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for fname in files:
                if not fname.endswith(".rs"):
                    continue
                full = os.path.join(folder, fname)
                rel = os.path.relpath(full, root).replace(os.sep, "/")
                if rel in seen:
                    continue
                seen.add(rel)
                try:
                    with open(full, encoding="utf-8", errors="replace") as f:
                        text = f.read()
                except OSError:
                    continue
                owner = innermost(rel, pkgs)
                here = posixpath.dirname(rel)
                found = [posixpath.normpath(posixpath.join(here, m)) for rx in (INCLUDE, PATH_ATTR) for m in rx.findall(text)]
                for m in MANIFEST_DIR.finditer(text):
                    found += [posixpath.normpath(posixpath.join(pkgs[owner]["dir"], lit))
                              for lit in DOTS.findall(text[m.end():m.end() + WINDOW])]
                for path in found:
                    # the workspace's root itself (a test's cwd) says nothing about a file: only a file or folder in it does
                    if path in (".", "") or path.startswith("../") or path == ".." or inside(path, pkgs[owner]["dir"]):
                        continue
                    refs.append((path, owner))
                for target in CARGO_P.findall(text):
                    if target in pkgs and target != owner:
                        edges.add((owner, target))
    return refs, edges


def ref_owners(path, refs):
    return {owner for ref, owner in refs if inside(path, ref)}


def full_reason(path, build_scripts):
    """What makes this file change the whole workspace's build, or None."""
    name = posixpath.basename(path)
    if name in FULL_NAMES:
        return FULL_NAMES[name]
    if path in build_scripts:
        return "build script"
    parts = path.split("/")
    for d, what in FULL_DIRS.items():
        if inside(path, d) or (len(d.split("/")) == 1 and d in parts):
            return what
    return None


def ignored(path):
    parts = path.split("/")
    if len(parts) == 1:
        return path in IGNORED_ROOT_FILES or path.endswith(".md")
    return parts[0] in IGNORED_DIRS


def closure(direct, deps):
    """The crates in `direct` and every crate that depends on one of them, however far."""
    reverse = {}
    for n, ds in deps.items():
        for d in ds:
            reverse.setdefault(d, set()).add(n)
    out, todo = set(direct), list(direct)
    while todo:
        for n in reverse.get(todo.pop(), ()):
            if n not in out:
                out.add(n)
                todo.append(n)
    return out


def with_packages(argv, names):
    """`--workspace` in a step's command becomes `-p <crate>` for each name."""
    if "--workspace" not in argv:
        return list(argv)
    i = argv.index("--workspace")
    return argv[:i] + [x for n in sorted(names) for x in ("-p", n)] + argv[i + 1:]


def pick_steps(templates, names, fmt_only=False):
    """The check's steps for these crates. A step with `only` runs when that crate is among them (and not in a fmt-only
    plan), one with `--workspace` for the listed crates alone, and the others (cargo fmt) as they are."""
    out = []
    for s in templates:
        if fmt_only and (s.get("only") or "--workspace" in s["argv"]):
            continue
        if s.get("only") and s["only"] not in names:
            continue
        out.append({**s, "argv": with_packages(s["argv"], names)})
    return out


def few(items, n=3):
    items = list(items)
    return ", ".join(items[:n]) + (f" (+{len(items) - n} more)" if len(items) > n else "")


def plan(templates, changed, meta, root):
    """The steps to run, and why. `templates` is the full check's steps (`only`: run for that crate alone; `--workspace`:
    replaced by the affected crates); `changed` is {"base", "merge_base", "files", "error"} from the PC; `meta` the
    workspace's `cargo metadata --no-deps` (None when it could not be had); `root` its sources.
    -> {"mode": "full" | "affected" | "fmt", "steps", "packages": [...], "total": crates in the workspace, "why", "lines": [what to print]}"""
    files = sorted(set(changed.get("files") or []))
    base = changed.get("base") or "?"
    sha = (changed.get("merge_base") or "")[:8]
    lines = [f"affected: {len(files)} file{' differs' if len(files) == 1 else 's differ'} from {base}"
             + (f" (merge-base {sha})" if sha else "")]

    def result(mode, steps, names, why, total, extra=()):
        return {"mode": mode, "steps": steps, "packages": sorted(names), "total": total, "why": why,
                "lines": lines + list(extra)}

    def full(why, total=0):
        return result("full", templates, [], why, total, [f"full check: {why}"])

    if changed.get("error"):
        return full(changed["error"])
    if not files:
        return full("no file differs from the base: nothing to map")
    if meta is None:
        return full("cargo metadata could not be read")
    try:
        pkgs, build_scripts = load(meta)
    except (KeyError, TypeError, ValueError) as e:
        return full(f"cargo metadata unreadable ({e!r})")
    total = len(pkgs)
    triggers = [(f, full_reason(f, build_scripts)) for f in files]
    triggers = [(f, why) for f, why in triggers if why]
    if triggers:
        return full(f"{few(f'{f} ({why})' for f, why in triggers)} changed, and that changes how every crate builds", total)

    refs, edges = scan(root, pkgs)
    direct, unmapped = {}, []
    for f in files:
        own = owners(f, pkgs)
        read = ref_owners(f, refs) - own
        for n in own:
            direct.setdefault(n, {"files": [], "reads": []})["files"].append(f)
        for n in read:
            direct.setdefault(n, {"files": [], "reads": []})["reads"].append(f)
        if not own and not read and not ignored(f):
            unmapped.append(f)
    if unmapped:
        one = len(unmapped) == 1
        return full(f"{few(unmapped)}: no crate owns or reads {'it' if one else 'them'} and {'it is' if one else 'they are'} not plainly "
                    f"documentation (IGNORED_DIRS in vm/affected.py), so what {'it reaches' if one else 'they reach'} is unknown", total)
    if not direct:
        return result("fmt", pick_steps(templates, set(), fmt_only=True), [], "only documentation and files no crate reads", total,
                      ["docs only: no crate is affected, only the formatting is checked"])

    deps = {n: set(p["deps"]) for n, p in pkgs.items()}
    for owner, target in edges:
        deps[owner].add(target)
    affected = closure(direct, deps)
    if affected == set(pkgs):
        return full("every crate is affected", total)
    out = [f"{len(affected)} of {total} crates: {few(sorted(affected), 8)}"]
    for n in sorted(affected):
        d = direct.get(n)
        why = []
        if d and d["files"]:
            why.append(f"{len(d['files'])} file{'' if len(d['files']) == 1 else 's'} changed ({few(d['files'], 2)})")
        if d and d["reads"]:
            why.append(f"reads {few(d['reads'], 2)}")
        uses = sorted(deps[n] & affected)
        if uses and not why:
            why.append(f"depends on {few(uses)}")
        out.append(f"  {n}: " + "; ".join(why))
    skipped = sorted({s["only"] for s in templates if s.get("only") and s["only"] not in affected})
    if skipped:
        out.append(f"steps for {', '.join(skipped)} skipped: not affected")
    return result("affected", pick_steps(templates, affected), affected, f"{len(affected)} of {total} crates", total, out)
