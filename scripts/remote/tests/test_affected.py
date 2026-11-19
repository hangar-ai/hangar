"""Tests of `rbuild check --affected`: the planning (vm/affected.py) and the changed files (hangar_rbuild.changed_files).

    python3 -B -m unittest discover -s scripts/remote/tests

No build machine and no cargo needed: the workspace is a small one made in a temporary folder, with the shape of
Hangar's (a theme crate below a ui crate, a core crate everything leans on, a nested probe crate, a crate that reads a
file of another and one whose tests run cargo on another). -B: a __pycache__ in vm/ would be shipped to the box.
"""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
REMOTE = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(REMOTE, "vm"))
import affected  # noqa: E402

spec = importlib.util.spec_from_file_location("hangar_rbuild", os.path.join(REMOTE, "hangar_rbuild.py"))
rbuild = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rbuild)

STEPS = rbuild.CHECK_STEPS

# name: (folder, [workspace crates it depends on])
CRATES = {
    "hangar-theme": ("crates/hangar-theme", []),
    "hangar-ui": ("crates/hangar-ui", ["hangar-theme"]),
    "hangar-core": ("crates/hangar-core", []),
    "hangar-engine": ("crates/hangar-engine", ["hangar-core"]),
    "hangar-webview": ("crates/hangar-webview", ["hangar-core"]),
    "hangar-kit-host": ("crates/hangar-kit-host", ["hangar-ui"]),
    "hangar-kit-probe": ("crates/hangar-kit-host/tests/probe", []),
    "hangar-app": ("crates/hangar-app", ["hangar-engine", "hangar-kit-host", "hangar-ui", "hangar-webview"]),
    "hangar-cloud": ("crates/hangar-cloud", []),
}


def metadata(root, crates=CRATES, build_scripts=()):
    packages = []
    for name, (folder, deps) in crates.items():
        targets = [{"name": name, "kind": ["lib"], "src_path": f"{root}/{folder}/src/lib.rs"}]
        if folder + "/build.rs" in build_scripts:
            targets.append({"name": "build-script-build", "kind": ["custom-build"], "src_path": f"{root}/{folder}/build.rs"})
        packages.append({"name": name, "manifest_path": f"{root}/{folder}/Cargo.toml", "targets": targets,
                         "dependencies": [{"name": d, "path": f"{root}/{crates[d][0]}"} for d in deps]
                         + [{"name": "serde", "path": None}]})
    return {"workspace_root": root, "packages": packages}


def write(root, rel, text=""):
    path = os.path.join(root, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


class Planning(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        for name, (folder, _) in CRATES.items():
            write(self.root, f"{folder}/src/lib.rs", "")
        # hangar-theme reads a file of hangar-core, as the real one reads kit-sdk/tokens.json
        write(self.root, "crates/hangar-theme/src/kit.rs", 'const T: &str = include_str!("../../hangar-core/kit-sdk/tokens.json");\n')
        # hangar-app's tests read an asset at the root, once with concat! and once with join; a chain of ".." is no path
        write(self.root, "crates/hangar-app/src/tests.rs",
              'include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/logo.png"));\n'
              'Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/a.glb");\n'
              'let m = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("Cargo.toml");\n')
        # hangar-core's tests read a folder of examples
        write(self.root, "crates/hangar-core/src/kits.rs", 'Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kits/hello")\n')
        # hangar-kit-host's tests run cargo on the probe and on a crate that is not below it
        write(self.root, "crates/hangar-kit-host/tests/tabs.rs", 'Command::new("cargo").args(["-p", "hangar-kit-probe", "-p", "hangar-cloud"]);\n')

    def tearDown(self):
        self._tmp.cleanup()

    def plan(self, files, meta=None, **changed):
        meta = metadata(self.root) if meta is None else meta
        return affected.plan(STEPS, {"base": "origin/main", "merge_base": "0123456789", "files": files, **changed}, meta, self.root)

    def names(self, result):
        return [s["name"] for s in result["steps"]]

    def test_a_leaf_crate_is_checked_with_the_crates_that_depend_on_it_however_far(self):
        r = self.plan(["crates/hangar-theme/src/palette.rs"])
        self.assertEqual(r["mode"], "affected")
        self.assertEqual(r["packages"], ["hangar-app", "hangar-kit-host", "hangar-theme", "hangar-ui"])
        self.assertEqual(r["total"], 9)

    def test_the_steps_are_the_checks_for_those_crates_alone(self):
        r = self.plan(["crates/hangar-theme/src/palette.rs"])
        self.assertEqual(self.names(r), ["fmt", "clippy", "test"])
        clippy, test = r["steps"][1]["argv"], r["steps"][2]["argv"]
        self.assertEqual(clippy, ["cargo", "clippy", "-p", "hangar-app", "-p", "hangar-kit-host", "-p", "hangar-theme", "-p", "hangar-ui",
                                  "--all-targets", "--locked", "--", "-D", "warnings"])
        self.assertEqual(test, ["cargo", "test", "-p", "hangar-app", "-p", "hangar-kit-host", "-p", "hangar-theme", "-p", "hangar-ui",
                                "--locked", "--no-fail-fast"])
        self.assertNotIn("--workspace", clippy + test)

    def test_the_headless_core_steps_run_only_when_core_is_affected(self):
        r = self.plan(["crates/hangar-core/src/lib.rs"])
        self.assertEqual(r["packages"], ["hangar-app", "hangar-core", "hangar-engine", "hangar-webview"])
        self.assertEqual(self.names(r), ["fmt", "clippy", "clippy core headless", "test core headless", "clippy webview engine", "test"])

    def test_the_webview_engine_step_runs_only_when_the_webview_is_affected(self):
        r = self.plan(["crates/hangar-webview/src/lib.rs"])
        self.assertEqual(self.names(r), ["fmt", "clippy", "clippy webview engine", "test"])

    def test_a_change_in_the_whole_workspace_is_the_full_check_itself(self):
        r = self.plan(["crates/hangar-core/src/lib.rs", "crates/hangar-theme/src/lib.rs", "crates/hangar-cloud/src/lib.rs",
                       "crates/hangar-kit-host/tests/probe/src/lib.rs"])
        self.assertEqual(r["mode"], "full")
        self.assertEqual(r["steps"], STEPS)
        self.assertIn("every crate", r["why"])

    def test_a_crate_nested_in_another_takes_its_parent_along(self):
        r = self.plan(["crates/hangar-kit-host/tests/probe/src/lib.rs"])
        self.assertEqual(r["packages"], ["hangar-app", "hangar-kit-host", "hangar-kit-probe"])

    def test_a_crate_whose_tests_run_cargo_on_another_is_affected_by_it(self):
        r = self.plan(["crates/hangar-cloud/src/main.rs"])
        self.assertEqual(r["packages"], ["hangar-app", "hangar-cloud", "hangar-kit-host"])

    def test_a_file_a_crate_includes_from_another_reaches_the_includer(self):
        r = self.plan(["crates/hangar-core/kit-sdk/tokens.json"])
        self.assertEqual(r["packages"], ["hangar-app", "hangar-core", "hangar-engine", "hangar-kit-host", "hangar-theme", "hangar-ui",
                                         "hangar-webview"])
        self.assertTrue(any("hangar-theme: reads crates/hangar-core/kit-sdk/tokens.json" in line for line in r["lines"]))

    def test_an_asset_and_a_folder_the_tests_read_by_a_relative_path(self):
        self.assertEqual(self.plan(["assets/logo.png"])["packages"], ["hangar-app"])
        self.assertEqual(self.plan(["assets/models/a.glb"])["packages"], ["hangar-app"])
        r = self.plan(["examples/kits/hello/kit.json"])
        self.assertIn("hangar-core", r["packages"])
        self.assertEqual(r["mode"], "affected")

    def test_a_chain_of_joins_is_no_path_and_the_workspace_root_is_not_a_file(self):
        refs, _ = affected.scan(self.root, affected.load(metadata(self.root))[0])
        self.assertNotIn(("crates", "hangar-app"), refs)
        self.assertFalse([r for r in refs if r[0] in ("", ".", "Cargo.toml")])

    def test_documentation_alone_checks_the_formatting_only(self):
        r = self.plan(["docs/native/remote-build.md", "site/src/index.html", "README.md", "mobile/src/a.ts", ".github/workflows/ci.yml",
                       "scripts/node-test/up.sh"])
        self.assertEqual(r["mode"], "fmt")
        self.assertEqual(self.names(r), ["fmt"])
        self.assertEqual(r["packages"], [])

    def test_what_makes_everything_build_differently_is_the_full_check(self):
        for path in ["Cargo.toml", "Cargo.lock", "crates/hangar-ui/Cargo.toml", "rust-toolchain.toml", "rust-toolchain", ".cargo/config.toml",
                     "crates/hangar-ui/.cargo/config.toml", "scripts/remote/rbuild", "scripts/remote/vm/remote.py", "crates/hangar-app/build.rs",
                     "rustfmt.toml", "clippy.toml"]:
            with self.subTest(path):
                r = self.plan([path, "crates/hangar-theme/src/lib.rs"])
                self.assertEqual(r["mode"], "full")
                self.assertEqual(r["steps"], STEPS)
                self.assertIn(path, r["why"])

    def test_a_build_script_under_another_name_is_found_in_the_metadata(self):
        meta = metadata(self.root, build_scripts=["crates/hangar-ui/build.rs"])
        for target in meta["packages"][1]["targets"]:
            if target["kind"] == ["custom-build"]:
                target["src_path"] = f"{self.root}/crates/hangar-ui/generate.rs"
        self.assertEqual(self.plan(["crates/hangar-ui/generate.rs"], meta)["mode"], "full")

    def test_a_file_no_crate_reads_and_that_is_not_documentation_is_the_full_check(self):
        r = self.plan(["config/new-thing.json", "crates/hangar-theme/src/lib.rs"])
        self.assertEqual(r["mode"], "full")
        self.assertIn("config/new-thing.json", r["why"])

    def test_a_markdown_file_in_a_crate_belongs_to_it(self):
        self.assertEqual(self.plan(["crates/hangar-theme/README.md"])["mode"], "affected")

    def test_what_cannot_be_read_or_compared_is_the_full_check(self):
        self.assertEqual(self.plan([], )["mode"], "full")
        r = self.plan(["crates/hangar-theme/src/lib.rs"], error="HEAD and origin/main have no merge-base")
        self.assertEqual((r["mode"], r["steps"]), ("full", STEPS))
        self.assertIn("no merge-base", r["why"])
        self.assertEqual(affected.plan(STEPS, {"base": "origin/main", "files": ["crates/hangar-ui/src/a.rs"]}, None, self.root)["mode"], "full")
        self.assertEqual(self.plan(["crates/hangar-ui/src/a.rs"], {"packages": [{"name": "x"}], "workspace_root": self.root})["mode"], "full")

    def test_a_deleted_file_is_mapped_by_its_path(self):
        self.assertEqual(self.plan(["crates/hangar-ui/src/gone.rs"])["packages"], ["hangar-app", "hangar-kit-host", "hangar-ui"])

    def test_the_lines_say_what_was_chosen_and_why(self):
        r = self.plan(["crates/hangar-theme/src/a.rs", "crates/hangar-theme/src/b.rs"])
        text = "\n".join(r["lines"])
        self.assertIn("2 files differ from origin/main (merge-base 01234567)", text)
        self.assertIn("4 of 9 crates", text)
        self.assertIn("hangar-theme: 2 files changed", text)
        self.assertIn("hangar-ui: depends on hangar-theme", text)
        self.assertIn("steps for hangar-core, hangar-webview skipped", text)

    def test_the_templates_are_left_as_they_were(self):
        before = json.dumps(STEPS)
        self.plan(["crates/hangar-theme/src/a.rs"])
        self.assertEqual(json.dumps(STEPS), before)

    def test_the_real_steps_are_what_the_planning_expects(self):
        self.assertEqual([s["name"] for s in STEPS if s.get("only")], ["clippy core headless", "test core headless", "clippy webview engine"])
        self.assertEqual([s["name"] for s in STEPS if "--workspace" in s["argv"]], ["clippy", "test"])


def git(cwd, *args):
    subprocess.run(["git", "-C", cwd, "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", *args],
                   check=True, capture_output=True)


class ChangedFiles(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.top = self._tmp.name
        git(self.top, "init", "-q", "-b", "main")
        for rel in ["crates/a/src/lib.rs", "crates/a/src/old.rs", "docs/a.md", "keep.txt"]:
            write(self.top, rel, "x\n")
        write(self.top, ".gitignore", "ignored.txt\n")
        git(self.top, "add", "-A")
        git(self.top, "commit", "-q", "-m", "base")
        git(self.top, "update-ref", "refs/remotes/origin/main", "HEAD")
        git(self.top, "switch", "-q", "-c", "work")

    def tearDown(self):
        self._tmp.cleanup()

    def test_committed_staged_unstaged_untracked_and_deleted_files_all_count_and_ignored_ones_do_not(self):
        write(self.top, "crates/a/src/lib.rs", "committed\n")
        git(self.top, "commit", "-q", "-am", "work")
        write(self.top, "docs/a.md", "staged\n")
        git(self.top, "add", "docs/a.md")
        write(self.top, "keep.txt", "unstaged\n")
        write(self.top, "crates/a/src/new.rs", "untracked\n")
        write(self.top, "ignored.txt", "ignored\n")
        os.remove(os.path.join(self.top, "crates/a/src/old.rs"))
        got = rbuild.changed_files(self.top)
        self.assertEqual(got["base"], "origin/main")
        self.assertEqual(got["files"], ["crates/a/src/lib.rs", "crates/a/src/new.rs", "crates/a/src/old.rs", "docs/a.md", "keep.txt"])
        self.assertEqual(len(got["merge_base"]), 40)

    def test_a_rename_lists_both_sides(self):
        git(self.top, "mv", "crates/a/src/old.rs", "docs/old.rs")
        git(self.top, "commit", "-q", "-m", "move")
        self.assertEqual(rbuild.changed_files(self.top)["files"], ["crates/a/src/old.rs", "docs/old.rs"])

    def test_it_is_the_merge_base_that_counts_not_what_the_base_gained_since(self):
        git(self.top, "switch", "-q", "main")
        write(self.top, "keep.txt", "main moved on\n")
        git(self.top, "commit", "-q", "-am", "main moves")
        git(self.top, "update-ref", "refs/remotes/origin/main", "HEAD")
        git(self.top, "switch", "-q", "work")
        write(self.top, "docs/a.md", "mine\n")
        git(self.top, "commit", "-q", "-am", "mine")
        self.assertEqual(rbuild.changed_files(self.top)["files"], ["docs/a.md"])

    def test_main_is_the_base_when_there_is_no_origin(self):
        git(self.top, "update-ref", "-d", "refs/remotes/origin/main")
        write(self.top, "keep.txt", "x2\n")
        got = rbuild.changed_files(self.top)
        self.assertEqual((got["base"], got["files"]), ("main", ["keep.txt"]))

    def test_nothing_to_compare_with_is_an_error_for_the_box_to_answer_with_the_full_check(self):
        git(self.top, "update-ref", "-d", "refs/remotes/origin/main")
        git(self.top, "branch", "-q", "-m", "main", "other")
        got = rbuild.changed_files(self.top)
        self.assertIn("error", got)
        self.assertNotIn("files", got)

    def test_a_base_that_is_not_a_commit_is_a_usage_error(self):
        with self.assertRaises(SystemExit) as raised:
            rbuild.changed_files(self.top, "no-such-branch")
        self.assertEqual(raised.exception.code, 2)

    def test_an_explicit_base_replaces_origin_main(self):
        write(self.top, "docs/a.md", "1\n")
        git(self.top, "commit", "-q", "-am", "one")
        write(self.top, "keep.txt", "2\n")
        git(self.top, "commit", "-q", "-am", "two")
        self.assertEqual(rbuild.changed_files(self.top, "HEAD~1")["files"], ["keep.txt"])


class Arguments(unittest.TestCase):
    def test_a_wrong_command_line_is_a_usage_error(self):
        for args in (["--bogus"], ["--base", "x"], ["--affected", "--base"], ["--affected", "--base="], ["--keep-going", "extra"]):
            with self.subTest(args):
                self.assertEqual(rbuild.cmd_check(args), 2)


if __name__ == "__main__":
    unittest.main()
