#!/usr/bin/env python3
"""A headless node measured from outside, on the binary itself.

    python3 scripts/node-bench/bench.py target/release/hangar-node [scenario…]

Each scenario starts `hangar-node serve` in a home of its own (nothing of the
account's is read or written), with the workspaces it needs already in its
state, and asks it over its API as a desktop would. Standard library only.

- start:  8 workspaces of 6 panes: until the node answers, until every shell
          runs, until every agent's command was typed; its memory then.
- memory: 16 panes at rest, then once each printed 12,000 lines (its screen's
          history and the node's own are full).
- mcp:    an agent's requests through Hangar's MCP server (`workspace_list`,
          `pane_list`), answered by the node with no desktop: each round trip.
- flood:  200 MB printed by one pane the node runs itself (its screen is fed)
          and by one it only runs for a desktop (no screen): time, and the
          processor the daemon spent.

Documented in docs/native/headless.md, "What it costs, measured".
"""

import http.client
import json
import os
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time

CLEANED = [
    "HANGAR_DATA_ROOT", "HANGAR_ENDPOINT_FILE", "HANGAR_PANE_ID", "HANGAR_PANE_NAME",
    "HANGAR_BRIDGE_URL", "HANGAR_BRIDGE_TOKEN", "HANGAR_PORT_BASE", "HANGAR_PORT_COUNT",
    "HANGAR_WORKSPACE", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME", "CLAUDE_CONFIG_DIR",
]
TICKS = os.sysconf("SC_CLK_TCK")


def wait(what, found, timeout=120.0, every=0.01):
    """Polls `found` until it answers something; the seconds it took."""
    began = time.monotonic()
    while True:
        value = found()
        if value:
            return time.monotonic() - began, value
        if time.monotonic() - began > timeout:
            raise SystemExit(f"never happened: {what}")
        time.sleep(every)


class Node:
    def __init__(self, binary, state=None):
        self.binary = os.path.abspath(binary)
        self.home = tempfile.mkdtemp(prefix="hangar-node-bench-")
        self.dir = os.path.join(self.home, ".local", "state", "hangar-node")
        os.makedirs(self.dir)
        if state is not None:
            with open(os.path.join(self.dir, "state.json"), "w") as out:
                json.dump(state, out)
        env = {k: v for k, v in os.environ.items() if k not in CLEANED}
        env["HOME"] = self.home
        self.env = env
        self.began = time.monotonic()
        self.process = subprocess.Popen(
            [self.binary, "serve"], env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=open(os.path.join(self.home, "node.log"), "w"),
            start_new_session=True,
        )
        self.port = self.token = None

    def ready(self):
        def answered():
            try:
                with open(os.path.join(self.dir, "node.json")) as file:
                    node = json.load(file)
                self.port, self.token = node["port"], node["token"]
                return self.call("GET", "/node/info")[0] == 200
            except (OSError, ValueError, KeyError, ConnectionError):
                return False
        wait("the node answers", answered)
        return time.monotonic() - self.began

    def call(self, method, path, body=None):
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=60)
        payload = json.dumps(body) if body is not None else None
        conn.request(method, path, body=payload, headers={
            "Authorization": f"Bearer {self.token}",
            "Content-Type": "application/json",
        })
        response = conn.getresponse()
        raw = response.read()
        conn.close()
        try:
            return response.status, json.loads(raw) if raw else None
        except ValueError:
            return response.status, None

    def panes(self):
        return self.call("GET", "/node/panes")[1]["panes"]

    def pss_mib(self):
        with open(f"/proc/{self.process.pid}/smaps_rollup") as file:
            for line in file:
                if line.startswith("Pss:"):
                    return int(line.split()[1]) / 1024
        return 0.0

    def cpu_s(self):
        with open(f"/proc/{self.process.pid}/stat") as file:
            fields = file.read().rsplit(")", 1)[1].split()
        return (int(fields[11]) + int(fields[12])) / TICKS

    def stop(self):
        try:
            os.killpg(self.process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            self.process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            os.killpg(self.process.pid, signal.SIGKILL)
        shutil.rmtree(self.home, ignore_errors=True)


def state_of(home_hint, workspaces, panes, agent, command=None, orchestrator=True):
    spaces = []
    for w in range(workspaces):
        cwd = os.path.join(home_hint, f"project-{w}")
        spaces.append({
            "id": f"w{w}", "name": f"Project {w}", "cwd": cwd,
            "panes": [
                dict(
                    {"id": f"w{w}-p{p}", "name": f"Agent {p}", "agent": agent},
                    **({"role": "orchestrator"} if orchestrator and agent == "claude" and p == 0 else {}),
                )
                for p in range(panes)
            ],
        })
    settings = {}
    if command:
        settings["agentCommands"] = {"claude": command}
    return {"workspaces": spaces, "settings": settings}


def with_folders(node_home, state):
    for ws in state["workspaces"]:
        os.makedirs(ws["cwd"], exist_ok=True)


def start_node(binary, build):
    """A node whose state `build(home)` makes; folders made before it starts."""
    home = tempfile.mkdtemp(prefix="hangar-node-bench-work-")
    state = build(home)
    with_folders(home, state)
    node = Node(binary, state)
    node.work = home
    return node


def scenario_start(binary):
    workspaces, panes = 8, 6
    marker = {}

    def build(home):
        fake = os.path.join(home, "fake-claude")
        marker["file"] = os.path.join(home, "started.txt")
        with open(fake, "w") as out:
            out.write(f"#!/bin/sh\necho \"$HANGAR_PANE_ID\" >> '{marker['file']}'\nexec sleep 100000\n")
        os.chmod(fake, 0o755)
        return state_of(home, workspaces, panes, "claude", command=fake)

    node = start_node(binary, build)
    try:
        ready = node.ready()
        total = workspaces * panes
        wait("every shell runs", lambda: sum(1 for p in node.panes() if p["alive"]) >= total)
        shells = time.monotonic() - node.began

        def typed():
            try:
                with open(marker["file"]) as file:
                    return len(set(file.read().split())) >= total
            except OSError:
                return False
        wait("every agent started", typed)
        agents = time.monotonic() - node.began
        time.sleep(3)
        print(f"start    {workspaces} workspaces of {panes} panes   the node answers in {ready*1000:6.0f} ms   "
              f"every shell runs after {shells:5.2f} s   every agent typed after {agents:5.2f} s")
        print(f"start    the daemon's memory with {total} agents started   {node.pss_mib():6.1f} MiB "
              f"({node.pss_mib()/total:.2f} MiB a pane)")
    finally:
        node.stop()
        shutil.rmtree(node.work, ignore_errors=True)


def scenario_memory(binary):
    workspaces, panes = 4, 4
    node = start_node(binary, lambda home: state_of(home, workspaces, panes, "shell"))
    try:
        node.ready()
        total = workspaces * panes
        wait("every shell runs", lambda: sum(1 for p in node.panes() if p["alive"]) >= total)
        time.sleep(3)
        rest = node.pss_mib()
        line = "x" * 80
        for pane in node.panes():
            node.call("POST", f"/node/panes/{pane['id']}/input",
                      {"data": f"seq 1 12000 | sed 's/$/ {line}/'\r"})
        expected = 12000 * 86
        wait("every pane printed", lambda: all(p["seq"] >= expected for p in node.panes()), timeout=300, every=0.2)
        time.sleep(2)
        full = node.pss_mib()
        print(f"memory   {total} shells at rest   {rest:6.1f} MiB ({rest/total:.2f} MiB a pane)   "
              f"once each printed 12,000 lines   {full:6.1f} MiB ({full/total:.2f} MiB a pane)")
    finally:
        node.stop()
        shutil.rmtree(node.work, ignore_errors=True)


FAKE_AGENT = r'''#!/usr/bin/env python3
import json, os, subprocess, sys, time
out = os.path.join(os.environ["HOME"], "mcp.json")
proxy = subprocess.Popen([{binary!r}, "--mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
def ask(n, method, params):
    proxy.stdin.write(json.dumps({{"jsonrpc": "2.0", "id": n, "method": method, "params": params}}) + "\n")
    proxy.stdin.flush()
    return json.loads(proxy.stdout.readline())
ask(1, "initialize", {{"protocolVersion": "2024-11-05", "capabilities": {{}}, "clientInfo": {{"name": "bench", "version": "1"}}}})
times = {{}}
for tool in ("workspace_list", "pane_list"):
    spent = []
    for n in range({calls}):
        began = time.perf_counter()
        answer = ask(10 + n, "tools/call", {{"name": tool, "arguments": {{}}}})
        spent.append((time.perf_counter() - began) * 1000)
        text = answer.get("result", {{}}).get("content", [{{}}])[0].get("text", "")
        if "not connected" in text or answer.get("result", {{}}).get("isError"):
            times["refused"] = text[:200]
    times[tool] = spent
json.dump(times, open(out + ".tmp", "w"))
os.rename(out + ".tmp", out)
time.sleep(100000)
'''


def scenario_mcp(binary):
    calls = 300

    def build(home):
        fake = os.path.join(home, "fake-claude")
        with open(fake, "w") as out:
            out.write(FAKE_AGENT.format(binary=os.path.abspath(binary), calls=calls))
        os.chmod(fake, 0o755)
        state = state_of(home, 4, 4, "claude", command=fake)
        # One agent asks; the others are plain shells beside it.
        for w, ws in enumerate(state["workspaces"]):
            for p, pane in enumerate(ws["panes"]):
                if (w, p) != (0, 0):
                    pane["agent"] = "shell"
        return state

    node = start_node(binary, build)
    try:
        node.ready()
        out = os.path.join(node.home, "mcp.json")
        wait("the agent's calls", lambda: os.path.exists(out), timeout=180, every=0.1)
        with open(out) as file:
            times = json.load(file)
        if "refused" in times:
            raise SystemExit(f"the node refused: {times['refused']}")
        for tool in ("workspace_list", "pane_list"):
            spent = sorted(times[tool])
            print(f"mcp      {tool:<15} {calls} calls by an agent, answered by the node   "
                  f"median {statistics.median(spent):5.2f} ms   95th {spent[int(len(spent)*0.95)]:5.2f} ms   "
                  f"slowest {spent[-1]:5.2f} ms")
    finally:
        node.stop()
        shutil.rmtree(node.work, ignore_errors=True)


def scenario_flood(binary):
    megabytes = 200
    # `yes` prints faster than any terminal reads: the time is the node's.
    line = "0123456789" * 9 + "abcdefghi"

    def build(home):
        return state_of(home, 1, 1, "shell")

    node = start_node(binary, build)
    try:
        node.ready()
        wait("the shell runs", lambda: sum(1 for p in node.panes() if p["alive"]) >= 1)
        time.sleep(1.5)
        results = []
        for kind in ("own", "desktop"):
            done = os.path.join(node.home, f"done-{kind}")
            command = f"yes {line} | head -c {megabytes * 1024 * 1024}; echo done > '{done}'"
            cpu, began = node.cpu_s(), time.monotonic()
            if kind == "own":
                node.call("POST", "/node/panes/w0-p0/input", {"data": command + "\r"})
            else:
                status, body = node.call("POST", "/node/panes", {
                    "id": "desk", "cwd": node.home, "cols": 120, "rows": 40,
                    "shell": {"program": "sh", "args": ["-c", command]},
                })
                if status != 200:
                    raise SystemExit(f"the pane did not start: {body}")
            wait(f"the {kind} pane printed", lambda: os.path.exists(done), timeout=600, every=0.05)
            wall = time.monotonic() - began
            time.sleep(0.5)
            results.append((kind, wall, node.cpu_s() - cpu))
        for kind, wall, cpu in results:
            what = "a pane the node runs itself (its screen fed)" if kind == "own" else "a pane run for a desktop (no screen)      "
            print(f"flood    {megabytes} MB printed by {what}   {megabytes/wall:6.1f} MB/s   "
                  f"{cpu:5.2f} s of the daemon's processor ({cpu*1000/megabytes:5.1f} ms a MB)")
    finally:
        node.stop()
        shutil.rmtree(node.work, ignore_errors=True)


SCENARIOS = {
    "start": scenario_start,
    "memory": scenario_memory,
    "mcp": scenario_mcp,
    "flood": scenario_flood,
}


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    binary = sys.argv[1]
    wanted = sys.argv[2:] or list(SCENARIOS)
    for name in wanted:
        SCENARIOS[name](binary)


if __name__ == "__main__":
    main()
