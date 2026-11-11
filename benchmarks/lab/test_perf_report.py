import hashlib
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from perf_report import METRICS, PROTOCOLS, generate, grouped, median_interval, read_json, summarize, write_json


def benchmark_report(revision, throughput):
    row = dict.fromkeys(METRICS)
    row.update(
        profile="tiny", payload_size=0, concurrency=1, repetition=1,
        requests=120, warmup_requests=20, target_rps=None,
        throughput_rps=throughput, p50_us=5, p95_us=6, p99_us=7,
        request_body_bytes=8, response_body_bytes=10,
    )
    return {
        "revision": revision,
        "working_tree_dirty": False,
        "build_rustc": "rustc test",
        "os": "test OS",
        "cpu": "test CPU",
        "config": {
            "repetitions": 1,
            "requests": 120,
            "warmup_requests": 20,
            "target_rps": None,
            "concurrency": [1],
            "profiles": [{"name": "tiny", "payload_size": 0}],
        },
        "rows": [{"protocol": protocol, **row} for protocol in PROTOCOLS],
    }


class StatisticsTest(unittest.TestCase):
    def test_interval_uses_independent_pairs(self):
        self.assertEqual(median_interval([1, 2, 3, 4, 5]), (None, None, None))
        self.assertEqual(median_interval([6, 2, 3, 4, 5, 1]), (1, 6, 0.96875))
        self.assertEqual(median_interval(range(12))[:2], (2, 9))

    def test_paired_change_and_unavailable_measurements(self):
        row = summarize([100, 200, 0, None], [110, 180, 10, 1000])
        self.assertEqual(row["paired_n"], 2)
        self.assertEqual(row["paired_median_delta_pct"], 0)
        self.assertEqual(row["baseline_n"], 3)
        self.assertEqual(row["candidate_n"], 4)
        self.assertIsNone(row["delta_ci_lower_pct"])
        self.assertEqual(summarize([], [2, 4])["candidate_median"], 3)
        self.assertIsNone(summarize([None], [None])["candidate_median"])

    def test_missing_duplicate_or_nonfinite_rows(self):
        report = benchmark_report("a", 100)
        row = report["rows"].pop()
        with self.assertRaisesRegex(ValueError, "missing workload"):
            grouped(report)
        report["rows"].extend([row, row])
        with self.assertRaisesRegex(ValueError, "duplicate workload"):
            grouped(report)
        report["rows"].pop()
        row["p99_us"] = float("nan")
        with self.assertRaisesRegex(ValueError, "invalid measurement p99_us"):
            grouped(report)


class ReportTest(unittest.TestCase):
    def setUp(self):
        temp = TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.out = Path(temp.name)
        config = b"requests = 120\n"
        (self.out / "config.toml").write_bytes(config)
        self.manifest = {
            "schema_version": 1,
            "revisions": {"baseline": "a", "candidate": "b"},
            "compiler": "rustc test",
            "runner": {"kind": "github-hosted"},
            "config_sha256": hashlib.sha256(config).hexdigest(),
            "harness_sha256": {"baseline": "same", "candidate": "same"},
            "comparison_unavailable": None,
            "orders": [["baseline", "candidate"], ["candidate", "baseline"]],
        }
        write_json(self.out / "manifest.json", self.manifest)
        for pair, order in enumerate(self.manifest["orders"], 1):
            for side in order:
                report = self.path(pair, side)
                report.parent.mkdir(parents=True)
                scale = 100 if side == "baseline" else 110
                write_json(report, benchmark_report(self.manifest["revisions"][side], pair * scale))

    def path(self, pair, side):
        return self.out / f"pair-{pair:02d}" / side / "results.json"

    def test_reports_show_paired_change_and_label_hosted_run(self):
        generate(self.out)
        text = (self.out / "summary.md").read_text(encoding="utf-8")
        self.assertIn("**Advisory.**", text)
        self.assertIn("+10.00%", text)
        self.assertTrue((self.out / "comparison.csv").is_file())
        lines = (self.out / "observations.csv").read_text(encoding="utf-8").splitlines()
        self.assertTrue(lines[7].startswith("2,1,candidate,"))
        report = read_json(self.out / "comparison.json")
        metric = next(row for row in report["rows"] if row["metric"] == "throughput_rps")
        self.assertEqual(metric["paired_n"], 2)
        self.assertIsNone(metric["delta_ci_upper_pct"])

    def test_candidate_only_omits_deltas(self):
        self.manifest["orders"] = [["candidate"]]
        self.manifest["comparison_unavailable"] = "The baseline revision has no benchmark harness."
        write_json(self.out / "manifest.json", self.manifest)
        generate(self.out)
        text = (self.out / "summary.md").read_text(encoding="utf-8")
        self.assertIn(self.manifest["comparison_unavailable"], text)
        self.assertNotIn("+10.00%", text)

    def test_refuses_mixed_environment_or_workload(self):
        path = self.path(2, "candidate")
        report = read_json(path)
        report["os"] = "different OS"
        write_json(path, report)
        with self.assertRaisesRegex(ValueError, "environment differs"):
            generate(self.out)
        report["os"] = "test OS"
        report["config"]["requests"] += 1
        write_json(path, report)
        with self.assertRaisesRegex(ValueError, "configuration .* differs"):
            generate(self.out)

    def test_refuses_changed_configuration_file(self):
        (self.out / "config.toml").write_bytes(b"requests = 20\n")
        with self.assertRaisesRegex(ValueError, "differs from the recorded configuration"):
            generate(self.out)

    def test_refuses_harness_or_order_mismatch(self):
        self.manifest["orders"][1] = self.manifest["orders"][0]
        write_json(self.out / "manifest.json", self.manifest)
        with self.assertRaisesRegex(ValueError, "order must alternate"):
            generate(self.out)
        self.manifest["harness_sha256"]["baseline"] = "different"
        write_json(self.out / "manifest.json", self.manifest)
        with self.assertRaisesRegex(ValueError, "fingerprints differ"):
            generate(self.out)


if __name__ == "__main__":
    unittest.main()
