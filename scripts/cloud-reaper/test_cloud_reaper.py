"""Tests of the backstop reaper (cloud_reaper.py), on fixture instance lists, a fake Vast and a fake Hetzner.

    python3 -B -m unittest discover -s scripts/cloud-reaper     (in the public repository)
    python3 -B -m unittest discover -s public/cloud-reaper      (in HangarAI's)

No network, no key, no port: the calls go to a fake that records them. What they prove: an "hgc-" instance past the
ceiling is destroyed; one within it is kept; an instance of rbuild ("hangar-rbuild …"), of another project, or
without a label is never destroyed, however old and whatever else its label contains, in a dry run or not. The same of
the hosted nodes' Hetzner servers: one named "hgn-…" and labelled "hangar-node=1" past its ceiling is deleted; any other
never is; a ceiling is believed up to 24 h 10 after the broker set it, so that a session extended past a day keeps its
server and a broker that stopped cannot keep one forever.
"""
import io
import json
import os
import re
import sys
import unittest
import urllib.error

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import cloud_reaper as reaper  # noqa: E402

NOW = 1_795_100_000.0
CEILING = reaper.CEILING_SECS
MIN = 60


def raw(id, label, age_secs, status="running"):
    """An instance as Vast's v1 list gives it, started `age_secs` before NOW (None: not started)."""
    return {"id": id, "label": label, "actual_status": status, "start_date": None if age_secs is None else NOW - age_secs}


# A year, a week: ages no machine of HangarAI Cloud ever has.
ANCIENT = 365 * 24 * 3600

# The account as the person has it: HangarAI Cloud's machines among their own.
HGC_PAST = [
    raw(31_000_001, "hgc-j_1", CEILING + 11 * MIN),
    raw(31_000_002, "hgc-j_2.2", CEILING + 1),  # a job's second machine, a second past
    raw(31_000_003, "hgc-j_3", CEILING),  # exactly at it: past it, as the server judges (now >= ceiling)
    raw(31_000_004, "hgc-j_4", ANCIENT, status="exited"),  # stopped long ago: destroyed, never kept stopped
    raw(31_000_005, "hgc-j_5", 2 * CEILING, status="loading"),
]
HGC_WITHIN = [
    raw(31_000_010, "hgc-j_10", 0),
    raw(31_000_011, "hgc-j_11", 42 * MIN),
    raw(31_000_012, "hgc-j_12.3", CEILING - 1),  # a second short of it
    # A machine the server legitimately keeps: it takes the next job at minute 100, with a 120 min limit, so its own
    # ceiling is past minute 230; the one-job ceiling (160 min) must not touch it.
    raw(31_000_013, "hgc-j_13", 161 * MIN),
    raw(31_000_014, "hgc-j_14", 200 * MIN),
    raw(31_000_015, "hgc-j_15", 230 * MIN),
    raw(31_000_016, "hgc-j_16", 580 * MIN),  # 9 h 40: six hours of jobs, the last one's limit, keep and margin
    raw(31_000_017, "hgc-j_17", 24 * 60 * MIN),  # a day of jobs, one after the other
]
HGC_UNAGED = [
    raw(31_000_020, "hgc-j_20", None, status="loading"),
]
# Never ours, every one of them old enough to be destroyed if the label were: only the prefix decides.
OTHERS = [
    raw(30_000_001, "hangar-rbuild idle 1795000000", ANCIENT),
    raw(30_000_002, "hangar-rbuild busy 1795000000", ANCIENT),
    raw(30_000_003, "hangar-rbuild-trial-1 boot 1795000000", ANCIENT),
    raw(30_000_004, "aps5 build", ANCIENT),
    raw(30_000_005, None, ANCIENT),
    raw(30_000_006, "", ANCIENT),
    raw(30_000_007, "   ", ANCIENT),
    raw(30_000_008, "hgc-", ANCIENT),  # the bare prefix, as the server's is_ours says
    raw(30_000_009, "hgc", ANCIENT),
    raw(30_000_010, "hgc j_1", ANCIENT),
    raw(30_000_011, "HGC-j_1", ANCIENT),
    raw(30_000_012, " hgc-j_1", ANCIENT),
    raw(30_000_013, "hangar-rbuild hgc-j_1", ANCIENT),  # contains hgc-, does not start with it
    raw(30_000_014, "my-hgc-box", ANCIENT),
    raw(30_000_015, "hangar-rbuild idle 1795000000", 0),  # young ones are not ours either
    raw(30_000_016, 12345, ANCIENT),  # a label that is not text
    raw(30_000_017, ["hgc-j_1"], ANCIENT),
]
ACCOUNT = HGC_PAST + HGC_WITHIN + HGC_UNAGED + OTHERS
OTHER_IDS = {str(r["id"]) for r in OTHERS}
PAST_IDS = {str(r["id"]) for r in HGC_PAST}


def instances(raws):
    return [reaper.Instance(r) for r in raws]


class FakeVast:
    """Vast's answers to urlopen, in pages of `page` instances, recording every call as (method, path)."""

    def __init__(self, raws, page=4, destroy_status=None, list_status=200):
        self.raws = raws
        self.page = page
        self.destroy_status = destroy_status or {}
        self.list_status = list_status
        self.calls = []
        self.requests = []

    def __call__(self, request, timeout=None):
        url = request.full_url
        path = url[len(reaper.API):]
        self.calls.append((request.get_method(), path.split("?")[0]))
        self.requests.append(request)
        if request.get_method() == "GET":
            # Vast answers the list at this path alone: without its last slash, a 301 (seen on 2026-11-20).
            if path.split("?")[0] != "/api/v1/instances/":
                raise urllib.error.HTTPError(url, 301, "Moved Permanently", {}, None)
            if self.list_status != 200:
                raise urllib.error.HTTPError(url, self.list_status, "no", {}, None)
            start = int(re.search(r"after_token=(\d+)", path).group(1)) if "after_token=" in path else 0
            chunk = self.raws[start : start + self.page]
            token = str(start + self.page) if start + self.page < len(self.raws) else None
            return Answer(200, json.dumps({"instances": chunk, "next_token": token}).encode())
        iid = int(re.fullmatch(r"/api/v0/instances/(\d+)/", path).group(1))
        status = self.destroy_status.get(iid, 200)
        if status >= 400:
            raise urllib.error.HTTPError(url, status, "no", {}, None)
        return Answer(status, b'{"success": true}')

    def deleted(self):
        return [int(re.search(r"(\d+)/$", path).group(1)) for method, path in self.calls if method == "DELETE"]


class Answer:
    def __init__(self, status, body):
        self.status = status
        self._body = body

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def read(self, limit=None):
        return self._body


class FakeHetzner:
    """Hetzner's answers to urlopen, in pages of `page` servers, recording every call as (method, path)."""

    def __init__(self, raws=(), page=3, delete_status=None, list_status=200):
        self.raws = list(raws)
        self.page = page
        self.delete_status = delete_status or {}
        self.list_status = list_status
        self.calls = []
        self.requests = []

    def __call__(self, request, timeout=None):
        url = request.full_url
        path = url[len(reaper.HETZNER_API) :]
        self.calls.append((request.get_method(), path.split("?")[0]))
        self.requests.append(request)
        if request.get_method() == "GET":
            if self.list_status != 200:
                raise urllib.error.HTTPError(url, self.list_status, "no", {}, None)
            assert "label_selector=hangar-node" in path, path
            page = int(re.search(r"[?&]page=(\d+)", path).group(1))
            start = (page - 1) * self.page
            chunk = self.raws[start : start + self.page]
            following = page + 1 if start + self.page < len(self.raws) else None
            body = {"servers": chunk, "meta": {"pagination": {"page": page, "next_page": following}}}
            return Answer(200, json.dumps(body).encode())
        sid = int(re.fullmatch(r"/servers/(\d+)", path).group(1))
        status = self.delete_status.get(sid, 200)
        if status >= 400:
            raise urllib.error.HTTPError(url, status, "no", {}, None)
        return Answer(status, b'{"action": {}}')

    def deleted(self):
        return [int(path.rsplit("/", 1)[1]) for method, path in self.calls if method == "DELETE"]


def both(vast, hetzner):
    """One urlopen for the two fakes, by the API's address."""

    def call(request, timeout=None):
        if request.full_url.startswith(reaper.HETZNER_API):
            return hetzner(request, timeout)
        return vast(request, timeout)

    return call


def run(fake, *argv, key="k" * 24, hetzner=None, token="h" * 32):
    out = io.StringIO()
    hetzner = FakeHetzner() if hetzner is None else hetzner
    env = {"VAST_API_KEY": key, "HETZNER_API_TOKEN": token}
    code = reaper.main(list(argv), env, NOW, both(fake, hetzner), out)
    return code, out.getvalue()


class Judging(unittest.TestCase):
    def test_hgc_past_the_ceiling_is_destroyed(self):
        destroy, _, _, _ = reaper.judge(instances(ACCOUNT), NOW)
        self.assertEqual({str(i.id) for i, _ in destroy}, PAST_IDS)

    def test_hgc_within_the_ceiling_is_kept(self):
        _, keep, _, _ = reaper.judge(instances(ACCOUNT), NOW)
        self.assertEqual({i.id for i, _ in keep}, {r["id"] for r in HGC_WITHIN})

    def test_the_ceiling_is_a_machine_s_life(self):
        self.assertEqual(CEILING, reaper.MACHINE_LIFE_MIN * MIN)
        self.assertEqual(reaper.hours(CEILING // 60), "29h20")

    def test_a_new_machine_takes_the_longest_job_with_its_hour_to_spare(self):
        # The server's rule: a job's limit, keep and margin, and an hour more, within the machine's life; a day-long
        # job on a new machine, its 3 minutes of start besides: 25 h 43.
        longest = 3 + reaper.MAX_TIME_LIMIT_MIN + reaper.MAX_KEEP_MIN + reaper.CEILING_MARGIN_MIN + 60
        self.assertLess(longest * MIN, CEILING)

    def test_a_machine_in_legitimate_use_is_kept_well_past_its_first_job_s_ceiling(self):
        one_job = (120 + 30 + 10) * MIN  # what the server's ceiling is for a machine's first job of 2 hours
        for minutes in (161, 200, 230, 580, 24 * 60, reaper.MACHINE_LIFE_MIN - 1):
            self.assertGreater(minutes * MIN, one_job)
            destroy, keep, _, _ = reaper.judge(instances([raw(1, "hgc-a", minutes * MIN)]), NOW)
            self.assertEqual(([i.id for i, _ in destroy], [i.id for i, _ in keep]), ([], [1]), minutes)
        destroy, _, _, _ = reaper.judge(instances([raw(1, "hgc-a", reaper.MACHINE_LIFE_MIN * MIN)]), NOW)
        self.assertEqual([i.id for i, _ in destroy], [1])

    def test_a_second_before_the_ceiling_is_kept_and_the_ceiling_itself_is_not(self):
        destroy, keep, _, _ = reaper.judge(instances([raw(1, "hgc-a", CEILING - 1), raw(2, "hgc-b", CEILING)]), NOW)
        self.assertEqual([i.id for i, _ in keep], [1])
        self.assertEqual([i.id for i, _ in destroy], [2])

    def test_rbuild_other_projects_and_unlabelled_are_never_destroyed_however_old(self):
        for other in OTHERS:
            for age in (0, CEILING - 1, CEILING, 10 * CEILING, ANCIENT):
                destroy, keep, unaged, others = reaper.judge(instances([dict(other, start_date=NOW - age)]), NOW)
                self.assertEqual((destroy, keep, unaged, others), ([], [], [], 1), (other["label"], age))

    def test_an_hgc_instance_without_a_start_time_is_left_alone_and_cannot_be_aged(self):
        for start in (None, 0, -5, "soon", True):
            r = dict(raw(1, "hgc-a", 0), start_date=start)
            destroy, keep, unaged, _ = reaper.judge(instances([r]), NOW)
            self.assertEqual((destroy, keep, len(unaged)), ([], [], 1), start)

    def test_nothing_not_ours_is_ever_in_the_destroy_list_whatever_the_label_or_age(self):
        labels = [r["label"] for r in OTHERS] + ["hgc-x", "hgc-x.2", "hgc-hangar-rbuild"]
        for label in labels:
            for age in (0, CEILING - 1, CEILING, ANCIENT):
                destroy, _, _, _ = reaper.judge(instances([raw(7, label, age)]), NOW)
                self.assertEqual(bool(destroy), reaper.is_ours(label) and age >= CEILING, (label, age))


class Destroying(unittest.TestCase):
    def test_a_live_run_deletes_the_hgc_past_the_ceiling_and_nothing_else(self):
        fake = FakeVast(ACCOUNT)
        code, out = run(fake)
        self.assertEqual(code, 0, out)
        self.assertEqual(set(fake.deleted()), {int(i) for i in PAST_IDS})
        self.assertEqual(len(fake.deleted()), len(PAST_IDS))
        self.assertFalse(OTHER_IDS & {str(i) for i in fake.deleted()})

    def test_the_list_is_read_page_by_page_until_the_last(self):
        fake = FakeVast(ACCOUNT, page=3)
        run(fake)
        gets = [call for call in fake.calls if call[0] == "GET"]
        self.assertEqual(len(gets), -(-len(ACCOUNT) // 3))
        self.assertEqual(set(fake.deleted()), {int(i) for i in PAST_IDS})

    def test_a_dry_run_says_what_it_would_destroy_and_deletes_nothing(self):
        fake = FakeVast(ACCOUNT)
        code, out = run(fake, "--dry-run")
        self.assertEqual(code, 0, out)
        self.assertEqual(fake.deleted(), [])
        self.assertTrue(all(method == "GET" for method, _ in fake.calls))
        for r in HGC_PAST:
            self.assertIn(f"would destroy {r['id']} ({r['label']})", out)
        for r in HGC_WITHIN:
            self.assertNotIn(f"would destroy {r['id']}", out)
        self.assertNotIn("destroyed ", out)

    def test_the_output_never_names_an_instance_that_is_not_ours_nor_shows_the_key(self):
        fake = FakeVast(ACCOUNT)
        key = "SECRET-" + "k" * 24
        for argv in ((), ("--dry-run",)):
            code, out = run(fake, *argv, key=key)
            self.assertNotIn(key, out)
            for r in OTHERS:
                self.assertNotIn(str(r["id"]), out)
                if isinstance(r["label"], str) and r["label"].strip():
                    self.assertNotIn(f"({r['label']})", out)  # how an instance is named: id (label)
            self.assertIn(f"{len(OTHERS)} others left alone", out)
        for request in fake.requests:
            self.assertEqual(request.get_header("Authorization"), "Bearer " + key)

    def test_a_destroy_that_fails_is_reported_and_the_others_are_still_tried(self):
        fake = FakeVast(ACCOUNT, destroy_status={31_000_001: 500, 31_000_002: 404})
        code, out = run(fake)
        self.assertEqual(code, 1)
        self.assertEqual(set(fake.deleted()), {int(i) for i in PAST_IDS})
        self.assertIn("could not destroy 31000001", out)
        self.assertIn("destroyed 31000002", out)  # 404: already gone, as asked
        self.assertNotIn("could not destroy 31000003", out)

    def test_a_list_that_cannot_be_read_destroys_nothing(self):
        fake = FakeVast(ACCOUNT, list_status=500)
        code, out = run(fake)
        self.assertEqual((code, fake.deleted()), (1, []))
        self.assertIn("nothing was destroyed", out)

    def test_an_answer_that_is_not_a_list_destroys_nothing(self):
        def odd(request, timeout=None):
            return Answer(200, b'{"unexpected": []}')

        code, out = run(odd)
        self.assertEqual(code, 1)
        self.assertIn("nothing was destroyed", out)

    def test_an_hgc_instance_without_a_start_time_is_warned_about_not_destroyed(self):
        fake = FakeVast(HGC_UNAGED)
        code, out = run(fake)
        self.assertEqual((code, fake.deleted()), (0, []))
        self.assertIn("::warning::", out)

    def test_no_key_no_call(self):
        fake = FakeVast(ACCOUNT)
        code, out = run(fake, key="  ")
        self.assertEqual((code, fake.calls), (1, []))
        self.assertIn("VAST_API_KEY is not set", out)

    def test_the_destroy_itself_refuses_what_is_not_ours_even_when_asked_directly(self):
        fake = FakeVast(ACCOUNT)
        vast = reaper.Vast("k" * 24, fake)
        for other in instances(OTHERS):
            with self.assertRaises(reaper.Refused):
                vast.destroy(other)
        for odd in ("../../x", "12; rm", "", None, "1" * 19):
            with self.assertRaises(reaper.Refused):
                vast.destroy(reaper.Instance(raw(odd, "hgc-j_1", ANCIENT)))
        self.assertEqual(fake.calls, [])


# ---------------------------------------------------------------------------------------------------- hosted nodes

NODE_CEILING = reaper.NODE_CEILING_SECS


def iso(secs):
    """A time as Hetzner writes it."""
    import datetime

    return datetime.datetime.fromtimestamp(secs, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S+00:00")


def server(id, name, age_secs, ceiling=None, labels=None, set_ago=None):
    """A server as Hetzner lists it, created `age_secs` before NOW (None: no creation time), its ceiling label
    `ceiling` seconds from NOW (None: no ceiling label), set `set_ago` seconds before NOW (None: no set time, as a
    broker before extensions without end labelled it)."""
    found = {"hangar-node": "1"} if labels is None else dict(labels)
    if ceiling is not None:
        found["hgn-ceiling"] = str(int(NOW + ceiling))
    if set_ago is not None:
        found["hgn-ceiling-set"] = str(int(NOW - set_ago))
    return {"id": id, "name": name, "labels": found, "created": None if age_secs is None else iso(NOW - age_secs)}


DAY = 24 * 3600


HGN_PAST = [
    server(70_000_001, "hgn-s1-1-1", 3 * 3600, ceiling=-1),  # its session ended 10 minutes and a second ago
    server(70_000_002, "hgn-s2-2-1", 600, ceiling=0),  # exactly at it: past it
    server(70_000_003, "hgn-s3-1-1", NODE_CEILING),  # no ceiling label: 24 h 10 since its creation
    server(70_000_004, "hgn-s4-1-2", NODE_CEILING + 60, ceiling=3600),  # no set time: 24 h 10 since its creation
    dict(server(70_000_005, "hgn-s5-1-1", NODE_CEILING), labels={"hangar-node": "1", "hgn-ceiling": "soon"}),
    # Its broker stopped: the label set 24 h 10 and a minute ago says an hour more, which is not believed.
    server(70_000_006, "hgn-s6-1-1", 3 * DAY, ceiling=3600, set_ago=NODE_CEILING + 60),
    # Extended past a day, then its end came: past the label's ceiling.
    server(70_000_007, "hgn-s7-1-1", 2 * DAY, ceiling=-1, set_ago=20 * 3600),
    # A set time in the future is not believed: judged by its creation.
    server(70_000_008, "hgn-s8-1-1", NODE_CEILING, ceiling=3600, set_ago=-3600),
    # No creation time, but a ceiling and its time: aged by them.
    server(70_000_009, "hgn-s9-1-1", None, ceiling=-60, set_ago=3600),
]
HGN_WITHIN = [
    server(70_000_010, "hgn-s10-1-1", 60, ceiling=11 * 3600),
    server(70_000_011, "hgn-s11-1-1", 23 * 3600, ceiling=1),  # extended to its 24 hours
    server(70_000_012, "hgn-s12-1-1", NODE_CEILING - 1),
    # Extended again and again: created 3 days ago, its last extension 19 hours ago to 5 hours from now. Believed,
    # without the creation's cap.
    server(70_000_013, "hgn-s13-1-1", 3 * DAY, ceiling=5 * 3600, set_ago=19 * 3600),
    # The furthest an extension sets: 24 h 10 after it, a second short of it now.
    server(70_000_014, "hgn-s14-1-1", 2 * DAY, ceiling=1, set_ago=NODE_CEILING - 1),
    # Set a few minutes ahead of this clock (the two clocks differ a little): believed.
    server(70_000_015, "hgn-s15-1-1", 2 * DAY, ceiling=24 * 3600, set_ago=-5 * 60),
    # No creation time, but a ceiling and its time.
    server(70_000_016, "hgn-s16-1-1", None, ceiling=3600, set_ago=3600),
]
HGN_UNAGED = [
    server(70_000_020, "hgn-s20-1-1", None, ceiling=-3600),  # no creation time, no set time
    server(70_000_021, "hgn-s21-1-1", None, ceiling=-3600, set_ago=-DAY),  # a set time not believed
]
# Never ours, every one of them past any ceiling: only the name AND the label decide.
NOT_NODES = [
    server(60_000_001, "aps5-build", 30 * 24 * 3600, ceiling=-3600),  # labelled, another name
    server(60_000_002, "hgn-by-hand", 30 * 24 * 3600, labels={}),  # the name, no label
    server(60_000_003, "hgn-x", 30 * 24 * 3600, labels={"hangar-node": "0"}),
    server(60_000_004, "hgn-", 30 * 24 * 3600),  # the bare prefix
    server(60_000_005, "HGN-s1", 30 * 24 * 3600),
    server(60_000_006, " hgn-s1", 30 * 24 * 3600),
    server(60_000_007, "hgn-s1.evil", 30 * 24 * 3600),
    server(60_000_008, "web", 30 * 24 * 3600, labels={"team": "site"}),
    dict(server(60_000_009, "hgn-s9", 30 * 24 * 3600), name=None),
    dict(server(60_000_010, "hgn-s10", 30 * 24 * 3600), labels=["hangar-node"]),
]
PROJECT = HGN_PAST + HGN_WITHIN + HGN_UNAGED + NOT_NODES
HGN_PAST_IDS = {r["id"] for r in HGN_PAST}
NOT_NODE_IDS = {r["id"] for r in NOT_NODES}


def nodes(raws):
    return [reaper.Node(r) for r in raws]


class JudgingNodes(unittest.TestCase):
    def test_hosted_nodes_past_their_ceiling_are_deleted_and_those_within_kept(self):
        delete, keep, unaged, others = reaper.judge_nodes(nodes(PROJECT), NOW)
        self.assertEqual({n.id for n, _ in delete}, HGN_PAST_IDS)
        self.assertEqual({n.id for n, _ in keep}, {r["id"] for r in HGN_WITHIN})
        self.assertEqual([n.id for n in unaged], [70_000_020, 70_000_021])
        self.assertEqual(others, len(NOT_NODES))

    def test_a_server_is_ours_by_its_name_and_its_label_both(self):
        for raw in NOT_NODES:
            for ceiling in (-30 * 24 * 3600, 0, None):
                r = dict(raw)
                if isinstance(r["labels"], dict) and ceiling is not None:
                    r["labels"] = dict(r["labels"], **{"hgn-ceiling": str(int(NOW + ceiling))})
                self.assertEqual(reaper.judge_nodes(nodes([r]), NOW), ([], [], [], 1), r)

    def test_the_longest_a_label_is_believed_is_a_day_ahead_and_its_margin(self):
        self.assertEqual(NODE_CEILING, (24 * 60 + 10) * 60)
        self.assertEqual(reaper.hours(NODE_CEILING // 60), "24h10")

    def test_a_label_is_believed_up_to_24h10_after_it_was_set_and_no_further(self):
        # A session extended for days: its server lives as long as each new label says.
        for days in (1, 3, 30):
            node = reaper.Node(server(1, "hgn-s1-1-1", days * DAY, ceiling=2 * 3600, set_ago=22 * 3600))
            self.assertEqual(node.ceiling(NOW), NOW + 2 * 3600)
        # A label further than 24 h 10 after its set time is cut there.
        node = reaper.Node(server(1, "hgn-s1-1-1", 60, ceiling=30 * DAY, set_ago=0))
        self.assertEqual(node.ceiling(NOW), NOW + NODE_CEILING)
        # A broker that stopped extending it: deleted at most 24 h 10 after the last label, whatever it says.
        node = reaper.Node(server(1, "hgn-s1-1-1", 60, ceiling=365 * DAY, set_ago=NODE_CEILING))
        self.assertEqual(reaper.judge_nodes([node], NOW)[0], [(node, 0.0)])
        # Times that are not numbers are not believed.
        for odd in ("soon", "", "-5", "1.5", "1" * 13):
            raw = server(1, "hgn-s1-1-1", 60, ceiling=3600)
            raw["labels"]["hgn-ceiling-set"] = odd
            self.assertEqual(reaper.Node(raw).ceiling(NOW), NOW + 3600, odd)

    def test_hetzner_s_times_are_read(self):
        self.assertEqual(reaper.seconds_of("2026-11-20T10:00:00+00:00"), reaper.seconds_of("2026-11-20T11:00:00+01:00"))
        self.assertEqual(reaper.seconds_of("2026-11-20T10:00:00Z"), reaper.seconds_of("2026-11-20T10:00:00.250+00:00"))
        for wrong in (None, "", "yesterday", "2026-11-20", "2026-13-20T10:00:00Z", 1795000000):
            self.assertIsNone(reaper.seconds_of(wrong), wrong)


class DeletingNodes(unittest.TestCase):
    def test_a_live_run_deletes_the_hosted_nodes_past_their_ceiling_and_nothing_else(self):
        hetzner = FakeHetzner(PROJECT)
        code, out = run(FakeVast([]), hetzner=hetzner)
        self.assertEqual(code, 0, out)
        self.assertEqual(set(hetzner.deleted()), HGN_PAST_IDS)
        self.assertEqual(len(hetzner.deleted()), len(HGN_PAST_IDS))
        self.assertFalse(NOT_NODE_IDS & set(hetzner.deleted()))
        gets = [call for call in hetzner.calls if call[0] == "GET"]
        self.assertEqual(len(gets), -(-len(PROJECT) // 3))

    def test_a_dry_run_says_what_it_would_delete_and_deletes_nothing(self):
        hetzner = FakeHetzner(PROJECT)
        code, out = run(FakeVast([]), "--dry-run", hetzner=hetzner)
        self.assertEqual(code, 0, out)
        self.assertEqual(hetzner.deleted(), [])
        self.assertTrue(all(method == "GET" for method, _ in hetzner.calls))
        for r in HGN_PAST:
            self.assertIn(f"would delete {r['id']} ({r['name']})", out)
        for r in HGN_WITHIN:
            self.assertNotIn(f"would delete {r['id']}", out)

    def test_the_output_never_names_a_server_that_is_not_ours_nor_shows_the_token(self):
        token = "SECRET-" + "h" * 32
        for argv in ((), ("--dry-run",)):
            hetzner = FakeHetzner(PROJECT)
            code, out = run(FakeVast([]), *argv, hetzner=hetzner, token=token)
            self.assertNotIn(token, out)
            for r in NOT_NODES:
                self.assertNotIn(str(r["id"]), out)
            self.assertIn(f"{len(NOT_NODES)} others left alone", out)
            for request in hetzner.requests:
                self.assertEqual(request.get_header("Authorization"), "Bearer " + token)

    def test_a_delete_that_fails_is_reported_and_the_others_are_still_tried(self):
        hetzner = FakeHetzner(PROJECT, delete_status={70_000_001: 500, 70_000_002: 404})
        code, out = run(FakeVast([]), hetzner=hetzner)
        self.assertEqual(code, 1)
        self.assertEqual(set(hetzner.deleted()), HGN_PAST_IDS)
        self.assertIn("could not delete 70000001", out)
        self.assertIn("deleted 70000002", out)

    def test_a_list_that_cannot_be_read_deletes_nothing_and_vast_is_still_reaped(self):
        hetzner = FakeHetzner(PROJECT, list_status=500)
        vast = FakeVast(ACCOUNT)
        code, out = run(vast, hetzner=hetzner)
        self.assertEqual((code, hetzner.deleted()), (1, []))
        self.assertIn("nothing was deleted", out)
        self.assertEqual(set(vast.deleted()), {int(i) for i in PAST_IDS})

    def test_no_token_no_call_and_vast_still_reaped(self):
        hetzner = FakeHetzner(PROJECT)
        vast = FakeVast(ACCOUNT)
        code, out = run(vast, hetzner=hetzner, token=" ")
        self.assertEqual((code, hetzner.calls), (1, []))
        self.assertIn("HETZNER_API_TOKEN is not set", out)
        self.assertEqual(set(vast.deleted()), {int(i) for i in PAST_IDS})

    def test_the_delete_itself_refuses_what_is_not_ours_even_when_asked_directly(self):
        hetzner = FakeHetzner(PROJECT)
        client = reaper.Hetzner("h" * 32, hetzner)
        for other in nodes(NOT_NODES):
            with self.assertRaises(reaper.Refused):
                client.delete(other)
        for odd in ("../../x", "12; rm", "", None, "1" * 19, True):
            with self.assertRaises(reaper.Refused):
                client.delete(reaper.Node(dict(server(1, "hgn-s1-1-1", 0), id=odd)))
        self.assertEqual(hetzner.calls, [])


class TheServersNumbers(unittest.TestCase):
    """The ceiling is made of the server's limits (cloud_reaper.py, "When"): if the server changes one, this must
    change with it, or the reaper would destroy a machine in legitimate use. Only where the server's source is
    (HangarAI's repository)."""

    ROOT = os.path.dirname(os.path.dirname(HERE))
    SRC = os.path.join(ROOT, "crates", "hangar-cloud", "src", "compute")
    API = os.path.join(ROOT, "crates", "hangar-cloud-api", "src", "compute.rs")
    LIFE = os.path.join(SRC, "life.rs")
    PROVIDER = os.path.join(SRC, "provider.rs")

    @staticmethod
    def read(path):
        with open(path, encoding="utf-8") as f:
            return f.read()

    @unittest.skipUnless(
        all(os.path.exists(p) for p in (API, LIFE)), "the account server's source is not in this repository"
    )
    def test_the_limits_are_the_servers(self):
        api, life = self.read(self.API), self.read(self.LIFE)
        found = {
            "limit": re.search(r"pub const MAX_TIME_LIMIT_SECS: u64 = (\d+) \* 60;", api),
            "keep": re.search(r"pub const MAX_KEEP_SECS: u64 = (\d+) \* 60;", api),
            "margin": re.search(r"pub const CEILING_MARGIN_MS: i64 = (\d+) \* 60_000;", life),
            "life": re.search(r"pub const MACHINE_LIFE_MS: i64 = (\d+) \* 60_000;", life),
        }
        missing = [name for name, match in found.items() if not match]
        self.assertFalse(missing, f"the server's constants {missing} changed shape: update this test")
        self.assertEqual(
            (
                reaper.MAX_TIME_LIMIT_MIN,
                reaper.MAX_KEEP_MIN,
                reaper.CEILING_MARGIN_MIN,
                reaper.MACHINE_LIFE_MIN,
            ),
            (
                int(found["limit"].group(1)),
                int(found["keep"].group(1)),
                int(found["margin"].group(1)),
                int(found["life"].group(1)),
            ),
        )

    NODES_API = os.path.join(ROOT, "crates", "hangar-cloud-api", "src", "nodes.rs")
    HETZNER = os.path.join(SRC, "hetzner.rs")

    @unittest.skipUnless(
        all(os.path.exists(p) for p in (NODES_API, HETZNER)), "the account server's source is not in this repository"
    )
    def test_the_hosted_nodes_limits_and_names_are_the_servers(self):
        api, hetzner = self.read(self.NODES_API), self.read(self.HETZNER)
        found = {
            "hours": re.search(r"pub const MAX_SESSION_HOURS: u32 = (\d+);", api),
            "margin": re.search(r"pub const CEILING_MARGIN_SECS: u64 = (\d+) \* 60;", api),
            "prefix": re.search(r'pub const NAME_PREFIX: &str = "([^"]+)";', hetzner),
            "label": re.search(r'pub const LABEL: &str = "([^"]+)";', hetzner),
            "ceiling": re.search(r'pub const CEILING_LABEL: &str = "([^"]+)";', hetzner),
            "set": re.search(r'pub const CEILING_SET_LABEL: &str = "([^"]+)";', hetzner),
        }
        missing = [name for name, match in found.items() if not match]
        self.assertFalse(missing, f"the server's constants {missing} changed shape: update this test")
        self.assertEqual(reaper.MAX_SESSION_MIN, int(found["hours"].group(1)) * 60)
        self.assertEqual(reaper.NODE_MARGIN_MIN, int(found["margin"].group(1)))
        self.assertEqual(
            (reaper.NODE_PREFIX, reaper.NODE_LABEL, reaper.CEILING_LABEL, reaper.CEILING_SET_LABEL),
            (found["prefix"].group(1), found["label"].group(1), found["ceiling"].group(1), found["set"].group(1)),
        )

    @unittest.skipUnless(os.path.exists(PROVIDER), "the account server's source is not in this repository")
    def test_the_prefix_is_the_servers(self):
        prefix = re.search(r'pub const LABEL_PREFIX: &str = "([^"]+)";', self.read(self.PROVIDER))
        self.assertTrue(prefix, "the server's constant changed shape: update this test")
        self.assertEqual(reaper.LABEL_PREFIX, prefix.group(1))


if __name__ == "__main__":
    unittest.main()
