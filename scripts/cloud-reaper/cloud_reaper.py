#!/usr/bin/env python3
"""The backstop reaper of HangarAI Cloud's machines: the build machines on Vast.ai, and the hosted nodes' servers on
Hetzner Cloud (the second part, below the first).

HangarAI Cloud rents its build machines on Vast.ai, each under a label "hgc-<job>" ("hgc-<job>.2" for a job's second
machine). The account server destroys them on time, and sweeps every minute for any that nothing ties to a job. This
script is the last layer, "rule 7" of "No machine runs for nothing": it runs on GitHub's free runners every 15
minutes, knows nothing of that server, and destroys the "hgc-" instances that are past their ceiling, so that a
machine cannot run for nothing even when the server is down.

    python3 -I scripts/cloud-reaper/cloud_reaper.py            destroy what is past its ceiling
    python3 -I scripts/cloud-reaper/cloud_reaper.py --dry-run  say what it would destroy, destroy nothing

VAST_API_KEY is read from the environment (a key restricted to instances) and written nowhere.

Hosted nodes. HangarAI Cloud rents a Hetzner Cloud server per hosted node session, named "hgn-<session>-<attempt>-<try>"
and labelled "hangar-node=1", with its ceiling in seconds since the epoch in the label "hgn-ceiling" (the session's end
and 10 minutes) and when the server set it in "hgn-ceiling-set". A session is extended any number of times, for as
long as its credits last, but each time to at most 24 hours from then: the server writes the new ceiling and its time
on the labels before the extension is made (refused when Hetzner does not take them). With HETZNER_API_TOKEN set (a
token of the project "hangarai-nodes"), this script deletes the servers that are named "hgn-…" AND labelled
"hangar-node=1" and are past their ceiling: the label's, believed up to 24 h 10 min after the time it was set and no
further, so that a server whose broker stopped (and so stopped extending it) is deleted at most 24 h 10 min after the
last extension, whatever its label says. A set time more than 10 minutes ahead of this script's clock is not believed.
A server without a believed set time (labelled by a server before it, or by hand) is judged as before: its label's
ceiling, never later than Hetzner's own creation time + 24 h 10 min; without the label "hgn-ceiling", or with one that
is not a number, by that second ceiling alone. A server that has neither a believed set time nor a creation time
cannot be aged: it is left alone, and a warning says so. A server with another name or without the label is never
deleted, never named in the output: counted, that is all. Both checks are made twice, once to choose and once more in
Hetzner.delete, right before the call.

What it touches. Only an instance whose label starts with "hgc-". The Vast account is the person's own, shared with
rbuild's "hangar-rbuild …" machines and others: an instance with any other label, or none, is never destroyed, and
neither is it named in the output (this runs in a public repository, whose logs are public): it is counted, that is
all. The prefix is checked twice, once to choose and once more in Vast.destroy, right before the call.

When. The server's ceiling for a machine (its job's time limit + its keep time + 10 minutes) is pushed back at each
job placed on the machine and at each keep, and it lives in the server's database, which this script cannot read;
the label does not carry it. So a fixed ceiling must not be tighter than the longest a machine can legitimately
live, or it would destroy a customer's job in progress. A license has no hour limit, so that longest is the server's
own: the machine's life, 29 h 20 min (MACHINE_LIFE_MS), counted from the machine's row, written before Vast is asked
for it. The server gives a machine a job only while that job's ceiling and an hour more stay within its life; a job it
cannot take gets a new machine. Counted from Vast's own start time (which comes after the row, so it never ages a
machine more than the server does), this ceiling is past the end of every job by an hour at least. The server's own
sweep, every minute, destroys a normal machine long before; this is only for when the server is down.
What it does not bound: a machine kept idle by repeated "Keep 30 min more" (the server's keep route checks the
credits, not the machine's life). No job runs on it then, so destroying it loses its warm cache and nothing else. A
tighter backstop needs the label to carry the ceiling, pushed back at each job and keep: a change of the server, not
of this script. test_cloud_reaper.py checks these numbers against the server's source.

An "hgc-" instance with no start time (Vast lists one that is still loading without it) cannot be aged: it is left
alone and a warning says so. It is not destroyed blind.

Exit status: 0 when every instance was judged and every destroy done (or none was needed); 1 when a key is not set,
a list could not be read, or a destroy failed (the others are still tried), so that the run shows red and GitHub says
so. Each provider is reaped on its own: one failing never keeps the other from being reaped.
"""
import argparse
import datetime
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

sys.dont_write_bytecode = True

LABEL_PREFIX = "hgc-"

# The server's limits, in minutes (crates/hangar-cloud-api/src/compute.rs and crates/hangar-cloud/src/compute/life.rs):
# the longest time limit a job may ask for, the longest keep after it, the ceiling's margin, and a machine's life, past
# which no job of its ever runs (see the top).
MAX_TIME_LIMIT_MIN = 24 * 60
MAX_KEEP_MIN = 30
CEILING_MARGIN_MIN = 10
MACHINE_LIFE_MIN = 29 * 60 + 20
CEILING_SECS = MACHINE_LIFE_MIN * 60

API = "https://console.vast.ai"
PAGE = 25  # v1's most
MAX_PAGES = 80  # more than this is not an account of build machines
TIMEOUT_SECS = 30


def hours(minutes):
    """Minutes as "29h20"."""
    minutes = int(minutes)
    return f"{minutes // 60}h{minutes % 60:02d}"


def is_ours(label):
    """Whether a label is one of HangarAI Cloud's: "hgc-" and something after it, exactly, as the server judges it
    (provider.rs, is_ours). " hgc-x", "HGC-x", "hgc-", "hangar-rbuild …" and no label at all are not."""
    return isinstance(label, str) and label.startswith(LABEL_PREFIX) and len(label) > len(LABEL_PREFIX)


class Instance:
    """What the reaper reads of one of Vast's instances."""

    def __init__(self, raw):
        self.id = raw.get("id")
        label = raw.get("label")
        self.label = label if isinstance(label, str) else None
        status = raw.get("actual_status")
        self.status = status if isinstance(status, str) else None
        start = raw.get("start_date")
        # Vast's own start time, in seconds since the epoch; 0, null or anything else is "not started yet".
        self.start = float(start) if isinstance(start, (int, float)) and not isinstance(start, bool) and start > 0 else None

    @property
    def ours(self):
        return is_ours(self.label)

    def shown(self):
        return f"{self.id} ({self.label[:48]})"


def judge(instances, now):
    """Sorts the account's instances. Returns (destroy, keep, unaged, others): the "hgc-" instances past the ceiling
    with the minutes they have lived, those within it, those that cannot be aged, and how many are not ours at all.
    An instance that is not ours is in none of the lists."""
    destroy, keep, unaged, others = [], [], [], 0
    for instance in instances:
        if not instance.ours:
            others += 1
        elif instance.start is None:
            unaged.append(instance)
        elif now - instance.start >= CEILING_SECS:
            destroy.append((instance, (now - instance.start) / 60))
        else:
            keep.append((instance, (now - instance.start) / 60))
    return destroy, keep, unaged, others


class VastError(Exception):
    """A call Vast did not answer as asked. Says the call and the status, never the key or an answer's body."""


class Refused(Exception):
    """A destroy that was not even tried: the instance is not this script's to touch."""


class Vast:
    """Vast's REST API, the two calls the reaper needs (the account server's client makes the same two)."""

    def __init__(self, key, urlopen=urllib.request.urlopen):
        self._key = key
        self._urlopen = urlopen

    def _call(self, method, path, query=""):
        request = urllib.request.Request(
            API + path + query, method=method, headers={"Authorization": "Bearer " + self._key, "Accept": "application/json"}
        )
        try:
            with self._urlopen(request, timeout=TIMEOUT_SECS) as answer:
                return answer.status, answer.read(16 * 1024 * 1024)
        except urllib.error.HTTPError as e:
            e.close()
            return e.code, b""
        except (urllib.error.URLError, OSError, TimeoutError) as e:
            raise VastError(f"{method} {path}: {type(e).__name__}") from None

    def instances(self):
        """Every instance of the account, page by page."""
        found, after = [], None
        for _ in range(MAX_PAGES):
            query = f"?limit={PAGE}" + (f"&after_token={urllib.parse.quote(after, safe='')}" if after else "")
            status, body = self._call("GET", "/api/v1/instances/", query)
            if status != 200:
                raise VastError(f"GET /api/v1/instances/: status {status}")
            try:
                page = json.loads(body)
                raws = page["instances"]
                after = page.get("next_token") or None
                found.extend(Instance(raw) for raw in raws if isinstance(raw, dict))
            except (ValueError, KeyError, TypeError, AttributeError):
                raise VastError("GET /api/v1/instances/: an answer that is not a list of instances") from None
            if not after:
                return found
        raise VastError(f"GET /api/v1/instances/: more than {MAX_PAGES} pages")

    def destroy(self, instance):
        """Destroys an instance (never stopped: a stopped one still pays its disk), only if it is ours: this is the
        last check before the call, whatever the caller chose."""
        if not instance.ours:
            raise Refused("the label is not an hgc- one")
        if not re.fullmatch(r"[0-9]{1,18}", str(instance.id)):
            raise Refused("the id is not a number")
        status, _ = self._call("DELETE", f"/api/v0/instances/{instance.id}/")
        # 404: already gone, which is what was asked.
        if status not in range(200, 300) and status != 404:
            raise VastError(f"DELETE /api/v0/instances/{instance.id}/: status {status}")


def reap(vast, now, dry_run, out):
    """One pass: the account listed, each instance judged, each one past its ceiling destroyed. Returns the exit
    status."""
    try:
        instances = vast.instances()
    except VastError as e:
        print(f"::error::cloud-reaper: {e}; nothing was destroyed", file=out)
        return 1
    destroy, keep, unaged, others = judge(instances, now)
    ceiling = hours(CEILING_SECS // 60)
    print(
        f"cloud-reaper: {len(instances)} instances on the account: {len(destroy) + len(keep) + len(unaged)} labelled "
        f"{LABEL_PREFIX}…, {others} others left alone (never named, never touched); ceiling {ceiling} since Vast "
        f"started an instance" + ("; dry run, nothing will be destroyed" if dry_run else ""),
        file=out,
    )
    for instance, minutes in keep:
        print(f"cloud-reaper: keep {instance.shown()}: {hours(minutes)} of {ceiling}", file=out)
    for instance in unaged:
        print(f"::warning::cloud-reaper: {instance.shown()} has no start time yet: it cannot be aged, left alone", file=out)
    failed = 0
    for instance, minutes in destroy:
        why = f"{hours(minutes)} since Vast started it, past the ceiling of {ceiling}"
        if dry_run:
            print(f"cloud-reaper: would destroy {instance.shown()}: {why}", file=out)
            continue
        try:
            vast.destroy(instance)
            print(f"cloud-reaper: destroyed {instance.shown()}: {why}", file=out)
        except (VastError, Refused) as e:
            failed += 1
            print(f"::error::cloud-reaper: could not destroy {instance.shown()}: {e}", file=out)
    return 1 if failed else 0


# ---------------------------------------------------------------------------------------------------- hosted nodes

NODE_PREFIX = "hgn-"
NODE_LABEL = "hangar-node"
CEILING_LABEL = "hgn-ceiling"
CEILING_SET_LABEL = "hgn-ceiling-set"
# The server's limits, in minutes (crates/hangar-cloud-api/src/nodes.rs): the furthest a session's end is from when it
# is set (its start, each extension), and the margin after its end within which its server goes (its parking
# included).
MAX_SESSION_MIN = 24 * 60
NODE_MARGIN_MIN = 10
# The furthest a label's ceiling is believed after the time it was set; for a label without that time, after Hetzner
# created the server.
NODE_CEILING_SECS = (MAX_SESSION_MIN + NODE_MARGIN_MIN) * 60
# A set time further ahead of this script's clock than this is not believed.
SET_SKEW_SECS = 10 * 60
HETZNER_API = "https://api.hetzner.cloud/v1"
HETZNER_PAGE = 50  # Hetzner's most
HETZNER_MAX_PAGES = 40  # more than this is not a project of hosted nodes
NODE_NAME = re.compile(r"[A-Za-z0-9_-]+")


def is_node(name, labels):
    """Whether a server is one of HangarAI Cloud's hosted nodes, as the server judges it (hetzner.rs, is_ours): named
    "hgn-" and something, of letters, digits, "-" and "_", AND labelled "hangar-node=1". Either alone is not."""
    return (
        isinstance(name, str)
        and name.startswith(NODE_PREFIX)
        and len(name) > len(NODE_PREFIX)
        and bool(NODE_NAME.fullmatch(name))
        and isinstance(labels, dict)
        and labels.get(NODE_LABEL) == "1"
    )


def seconds_of(text):
    """Hetzner's time ("2026-11-20T10:00:00+00:00", with or without a fraction, "Z" too) in seconds since the epoch;
    None for anything else."""
    if not isinstance(text, str):
        return None
    m = re.fullmatch(r"(\d{4})-(\d\d)-(\d\d)[T ](\d\d):(\d\d):(\d\d)(?:\.\d+)?(Z|z|[+-]\d\d:\d\d)?", text.strip())
    if not m:
        return None
    try:
        when = datetime.datetime(*(int(m.group(i)) for i in range(1, 7)), tzinfo=datetime.timezone.utc)
    except ValueError:
        return None
    zone = m.group(7)
    offset = 0
    if zone and zone not in ("Z", "z"):
        sign = 1 if zone[0] == "+" else -1
        offset = sign * (int(zone[1:3]) * 3600 + int(zone[4:6]) * 60)
    return when.timestamp() - offset


def seconds_label(value):
    """A label's time in seconds since the epoch: digits, 12 at most; None for anything else."""
    return int(value) if isinstance(value, str) and re.fullmatch(r"[0-9]{1,12}", value) else None


class Node:
    """What the reaper reads of one of Hetzner's servers."""

    def __init__(self, raw):
        self.id = raw.get("id")
        name = raw.get("name")
        self.name = name if isinstance(name, str) else None
        labels = raw.get("labels")
        self.labels = labels if isinstance(labels, dict) else {}
        self.created = seconds_of(raw.get("created"))
        self.labelled = seconds_label(self.labels.get(CEILING_LABEL))
        self.labelled_at = seconds_label(self.labels.get(CEILING_SET_LABEL))

    @property
    def ours(self):
        return is_node(self.name, self.labels)

    def ceiling(self, now):
        """When it is past: its label's ceiling, never later than 24 h 10 min after the label was set; for a label
        without a believed set time, never later than its creation + 24 h 10 min. None when it cannot be aged."""
        if self.labelled is not None and self.labelled_at is not None and self.labelled_at <= now + SET_SKEW_SECS:
            return min(self.labelled, self.labelled_at + NODE_CEILING_SECS)
        if self.created is None:
            return None
        most = self.created + NODE_CEILING_SECS
        return most if self.labelled is None else min(self.labelled, most)

    def shown(self):
        return f"{self.id} ({self.name[:48]})"


def judge_nodes(nodes, now):
    """Sorts the project's servers. Returns (delete, keep, unaged, others): the hosted nodes past their ceiling with the
    minutes they are past it, those within it with the minutes left, those that cannot be aged, and how many are not
    ours at all. A server that is not ours is in none of the lists."""
    delete, keep, unaged, others = [], [], [], 0
    for node in nodes:
        if not node.ours:
            others += 1
            continue
        ceiling = node.ceiling(now)
        if ceiling is None:
            unaged.append(node)
        elif now >= ceiling:
            delete.append((node, (now - ceiling) / 60))
        else:
            keep.append((node, (ceiling - now) / 60))
    return delete, keep, unaged, others


class HetznerError(Exception):
    """A call Hetzner did not answer as asked. Says the call and the status, never the token or an answer's body."""


class Hetzner:
    """Hetzner Cloud's API, the two calls the reaper needs (the account server's client makes the same two)."""

    def __init__(self, token, urlopen=urllib.request.urlopen):
        self._token = token
        self._urlopen = urlopen

    def _call(self, method, path, query=""):
        request = urllib.request.Request(
            HETZNER_API + path + query,
            method=method,
            headers={"Authorization": "Bearer " + self._token, "Accept": "application/json"},
        )
        try:
            with self._urlopen(request, timeout=TIMEOUT_SECS) as answer:
                return answer.status, answer.read(16 * 1024 * 1024)
        except urllib.error.HTTPError as e:
            e.close()
            return e.code, b""
        except (urllib.error.URLError, OSError, TimeoutError) as e:
            raise HetznerError(f"{method} {path}: {type(e).__name__}") from None

    def servers(self):
        """Every server labelled hangar-node, page by page."""
        found, page = [], 1
        for _ in range(HETZNER_MAX_PAGES):
            query = f"?label_selector={NODE_LABEL}&page={page}&per_page={HETZNER_PAGE}"
            status, body = self._call("GET", "/servers", query)
            if status != 200:
                raise HetznerError(f"GET /servers: status {status}")
            try:
                answer = json.loads(body)
                raws = answer["servers"]
                next_page = ((answer.get("meta") or {}).get("pagination") or {}).get("next_page")
                found.extend(Node(raw) for raw in raws if isinstance(raw, dict))
            except (ValueError, KeyError, TypeError, AttributeError):
                raise HetznerError("GET /servers: an answer that is not a list of servers") from None
            if not isinstance(next_page, int) or next_page <= page:
                return found
            page = next_page
        raise HetznerError(f"GET /servers: more than {HETZNER_MAX_PAGES} pages")

    def delete(self, node):
        """Deletes a server (never powered off: Hetzner bills one that is off), only if it is ours: this is the last
        check before the call, whatever the caller chose."""
        if not node.ours:
            raise Refused("the server is not named hgn- and labelled hangar-node")
        if not re.fullmatch(r"[0-9]{1,18}", str(node.id)) or isinstance(node.id, bool):
            raise Refused("the id is not a number")
        status, _ = self._call("DELETE", f"/servers/{node.id}")
        # 404: already gone, which is what was asked.
        if status not in range(200, 300) and status != 404:
            raise HetznerError(f"DELETE /servers/{node.id}: status {status}")


def reap_nodes(hetzner, now, dry_run, out):
    """One pass over the hosted nodes: the project listed, each server judged, each one past its ceiling deleted.
    Returns the exit status."""
    try:
        nodes = hetzner.servers()
    except HetznerError as e:
        print(f"::error::cloud-reaper: {e}; nothing was deleted", file=out)
        return 1
    delete, keep, unaged, others = judge_nodes(nodes, now)
    print(
        f"cloud-reaper: {len(nodes)} servers labelled {NODE_LABEL} in the project: {len(delete) + len(keep) + len(unaged)} "
        f"hosted nodes ({NODE_PREFIX}…), {others} others left alone (never named, never touched); ceiling their label's, "
        f"at most {hours(NODE_CEILING_SECS // 60)} after it was set (after Hetzner created them, for a label without "
        f"its time)"
        + ("; dry run, nothing will be deleted" if dry_run else ""),
        file=out,
    )
    for node, minutes in keep:
        print(f"cloud-reaper: keep {node.shown()}: {hours(minutes)} left", file=out)
    for node in unaged:
        print(
            f"::warning::cloud-reaper: {node.shown()} has no creation time nor a ceiling set time: it cannot be aged, "
            f"left alone",
            file=out,
        )
    failed = 0
    for node, minutes in delete:
        why = f"{hours(minutes)} past its ceiling"
        if dry_run:
            print(f"cloud-reaper: would delete {node.shown()}: {why}", file=out)
            continue
        try:
            hetzner.delete(node)
            print(f"cloud-reaper: deleted {node.shown()}: {why}", file=out)
        except (HetznerError, Refused) as e:
            failed += 1
            print(f"::error::cloud-reaper: could not delete {node.shown()}: {e}", file=out)
    return 1 if failed else 0


def main(argv=None, env=None, now=None, urlopen=urllib.request.urlopen, out=None):
    parser = argparse.ArgumentParser(
        description="Destroys HangarAI Cloud's hgc- Vast instances and deletes its hgn- Hetzner servers past their ceiling."
    )
    parser.add_argument("--dry-run", action="store_true", help="print what would be destroyed, destroy nothing")
    args = parser.parse_args(argv)
    env = os.environ if env is None else env
    out = sys.stdout if out is None else out
    now = time.time() if now is None else now
    status = 0
    key = (env.get("VAST_API_KEY") or "").strip()
    if key:
        status |= reap(Vast(key, urlopen), now, args.dry_run, out)
    else:
        print("::error::cloud-reaper: VAST_API_KEY is not set", file=out)
        status = 1
    token = (env.get("HETZNER_API_TOKEN") or "").strip()
    if token:
        status |= reap_nodes(Hetzner(token, urlopen), now, args.dry_run, out)
    else:
        print("::error::cloud-reaper: HETZNER_API_TOKEN is not set", file=out)
        status = 1
    return status


if __name__ == "__main__":
    sys.exit(main())
