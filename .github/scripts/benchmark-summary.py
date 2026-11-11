"""Summarize benchmark JSON using medians across repetitions."""

import argparse
import json
from collections import defaultdict
from pathlib import Path
from statistics import median


def read_report(path):
    return json.loads(path.read_text(encoding="utf-8"))


def workloads(report):
    groups = defaultdict(list)
    for row in report["rows"]:
        key = (row["protocol"], row["profile"], row["payload_size"], row["concurrency"])
        groups[key].append(row)
    if not groups:
        raise ValueError("benchmark report has no measurements")
    return groups


def comparison_problem(candidate, baseline, candidate_groups, baseline_groups):
    for field in ("config", "rustc", "os", "cpu"):
        if candidate[field] != baseline[field]:
            return f"the reports differ in {field}"
    if candidate_groups.keys() != baseline_groups.keys():
        return "the reports contain different workloads"
    expected = list(range(1, candidate["config"]["repetitions"] + 1))
    for key, rows in candidate_groups.items():
        for group in (rows, baseline_groups[key]):
            if sorted(row["repetition"] for row in group) != expected:
                return "the reports have incomplete or duplicate repetitions"
    return None


def revision(report):
    return report.get("revision") or "unknown"


def metrics(rows):
    return (
        median(row["throughput_rps"] for row in rows),
        median(row["p95_us"] for row in rows),
    )


def delta(current, baseline):
    if baseline == 0:
        return "n/a"
    return f"{100 * (current - baseline) / baseline:+.1f}%"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--baseline", type=Path, help="directory containing main's report")
    args = parser.parse_args()

    candidate = read_report(args.report)
    candidate_groups = workloads(candidate)
    baseline_groups = None
    print(f"### Benchmarks · {args.report.parent.name}\n")
    print(f"Measured revision: `{revision(candidate)}`\n")

    if args.baseline:
        status_path = args.baseline / "status.txt"
        if status_path.is_file():
            sha = (args.baseline / "revision.txt").read_text(encoding="utf-8").strip()
            status = status_path.read_text(encoding="utf-8").strip()
            print(f"Comparison unavailable for main revision `{sha}`. {status}\n")
        else:
            baseline = read_report(args.baseline / "results.json")
            print(f"Main baseline: `{revision(baseline)}`\n")
            baseline_groups = workloads(baseline)
            problem = comparison_problem(candidate, baseline, candidate_groups, baseline_groups)
            if problem:
                print(f"Comparison unavailable because {problem}.\n")
                baseline_groups = None

    print("Values are medians across repetitions. On hosted runners, treat deltas as advisory.\n")
    headers = ["Protocol", "Profile", "Payload (B)", "Connections", "req/s", "p95 (µs)"]
    if baseline_groups is not None:
        headers += ["Main req/s", "Δ req/s", "Main p95 (µs)", "Δ p95"]
    print("| " + " | ".join(headers) + " |")
    print("| " + " | ".join(["---"] * len(headers)) + " |")
    for key, rows in sorted(candidate_groups.items()):
        throughput, p95 = metrics(rows)
        cells = [*map(str, key), f"{throughput:,.0f}", f"{p95:,.1f}"]
        if baseline_groups is not None:
            base_throughput, base_p95 = metrics(baseline_groups[key])
            cells += [
                f"{base_throughput:,.0f}",
                delta(throughput, base_throughput),
                f"{base_p95:,.1f}",
                delta(p95, base_p95),
            ]
        print("| " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
