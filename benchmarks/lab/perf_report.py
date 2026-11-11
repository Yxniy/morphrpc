"""Validate paired benchmark runs and write JSON, CSV and Markdown reports."""

import csv
import hashlib
import json
from collections import defaultdict
from html import escape
from math import comb, isfinite
from statistics import median, stdev


TIMING = {
    "throughput_rps": "Throughput (req/s)",
    "p50_us": "p50 (µs)",
    "p95_us": "p95 (µs)",
    "p99_us": "p99 (µs)",
}
OPTIONAL_TIMING = {
    f"scheduled_p{p}_us": f"Scheduled p{p} (µs)" for p in (50, 95, 99)
}
SIZES = {
    "request_body_bytes": "Request body (B)",
    "response_body_bytes": "Response body (B)",
    "request_framing_bytes": "Request framing (B)",
    "response_framing_bytes": "Response framing (B)",
    "transport_bytes": "Transport (B)",
    "server_rss_bytes": "Server RSS snapshot (B)",
    "cpu_usage": "CPU usage",
}
METRICS = {**TIMING, **OPTIONAL_TIMING, **SIZES}
PROTOCOLS = ("morphrpc", "grpc", "http")


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def write_csv(path, rows):
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, rows[0].keys(), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def grouped(report):
    cfg = report["config"]
    reps = cfg["repetitions"]
    expected = {
        (protocol, profile["name"], profile["payload_size"], connections, rep)
        for protocol in PROTOCOLS
        for profile in cfg["profiles"]
        for connections in cfg["concurrency"]
        for rep in range(1, reps + 1)
    }
    count = len(PROTOCOLS) * len(cfg["profiles"]) * len(cfg["concurrency"]) * reps
    if not expected or len(expected) != count:
        raise ValueError("empty or duplicate workloads in benchmark configuration")
    groups = defaultdict(list)
    for row in report["rows"]:
        key = (row["protocol"], row["profile"], row["payload_size"], row["concurrency"])
        identity = (*key, row["repetition"])
        if identity not in expected:
            raise ValueError(f"unexpected or duplicate workload row {identity}")
        expected.remove(identity)
        for field in ("requests", "warmup_requests", "target_rps"):
            if row[field] != cfg[field]:
                raise ValueError(f"workload row differs from configuration in {field}")
        for field in METRICS:
            value = row[field]
            required = (
                field in TIMING or field in ("request_body_bytes", "response_body_bytes")
                or (field in OPTIONAL_TIMING and cfg["target_rps"] is not None)
            )
            if value is None and not required:
                continue
            if type(value) not in (float, int) or not isfinite(value) or value < 0:
                raise ValueError(f"invalid measurement {field} for {identity}")
        if row["throughput_rps"] <= 0:
            raise ValueError(f"nonpositive throughput for {identity}")
        groups[key].append(row)
    if expected:
        raise ValueError("benchmark report has missing workload rows")
    return groups


def median_interval(values):
    """Central order-statistic interval with at least 95% binomial coverage."""
    ordered = sorted(values)
    n = len(ordered)
    outcomes = 2 ** n
    tail = 0
    interval = (None, None, None)
    for k in range(n // 2):
        tail += comb(n, k)
        if 40 * tail > outcomes:
            break
        interval = (ordered[k], ordered[-k - 1], 1 - 2 * tail / outcomes)
    return interval


def summarize(baseline, candidate):
    base_values = [v for v in baseline if v is not None]
    cand_values = [v for v in candidate if v is not None]
    changes = [
        100 * (c - b) / b
        for b, c in zip(baseline, candidate)
        if b is not None and b != 0 and c is not None
    ]
    low, high, coverage = median_interval(changes)
    return {
        "baseline_n": len(base_values),
        "candidate_n": len(cand_values),
        "baseline_median": median(base_values) if base_values else None,
        "candidate_median": median(cand_values) if cand_values else None,
        "baseline_sd": stdev(base_values) if len(base_values) >= 2 else None,
        "candidate_sd": stdev(cand_values) if len(cand_values) >= 2 else None,
        "paired_n": len(changes),
        "paired_median_delta_pct": median(changes) if changes else None,
        "delta_ci_lower_pct": low,
        "delta_ci_upper_pct": high,
        "delta_ci_coverage": coverage,
    }


def load_runs(out, manifest):
    if manifest["schema_version"] != 1:
        raise ValueError("unsupported manifest schema")
    digest = hashlib.sha256((out / "config.toml").read_bytes()).hexdigest()
    if digest != manifest["config_sha256"]:
        raise ValueError("config.toml differs from the recorded configuration")
    orders = manifest["orders"]
    unavailable = manifest["comparison_unavailable"]
    if unavailable:
        if orders != [["candidate"]]:
            raise ValueError("candidate-only report must contain exactly one run")
    else:
        fingerprints = manifest["harness_sha256"]
        if not fingerprints["baseline"] or fingerprints["baseline"] != fingerprints["candidate"]:
            raise ValueError("benchmark harness fingerprints differ")
        if len(orders) < 2 or len(orders) % 2:
            raise ValueError("comparison must have a balanced number of pairs")
        for i, order in enumerate(orders):
            expected = ["candidate", "baseline"] if i % 2 else ["baseline", "candidate"]
            if order != expected:
                raise ValueError("baseline and candidate order must alternate")

    metadata = None
    observations = []
    lockfiles = {}
    samples = defaultdict(lambda: defaultdict(list))
    for pair, order in enumerate(orders, start=1):
        for position, side in enumerate(order, start=1):
            path = out / f"pair-{pair:02d}" / side / "results.json"
            report = read_json(path)
            if (
                report["revision"] != manifest["revisions"][side]
                or report["working_tree_dirty"] is not False
                or report["build_rustc"] != manifest["compiler"]
            ):
                raise ValueError(f"wrong revision, dirty worktree or build compiler in {path}")
            current = {key: report[key] for key in ("config", "os", "cpu", "build_rustc")}
            if metadata is not None and metadata != current:
                raise ValueError(f"workload configuration or environment differs in {path}")
            metadata = current
            lock_hash = hashlib.sha256(report["cargo_lock"].encode("utf-8")).hexdigest()
            if side in lockfiles and lockfiles[side] != lock_hash:
                raise ValueError(f"lockfile differs between runs of {side}")
            lockfiles[side] = lock_hash
            for key, rows in grouped(report).items():
                for field in METRICS:
                    values = [row[field] for row in rows]
                    value = median(values) if all(v is not None for v in values) else None
                    samples[key][side, field].append(value)
                observations.extend(
                    {"pair": pair, "position": position, "side": side, "revision": report["revision"], **row}
                    for row in rows
                )

    metadata["cargo_lock_sha256"] = lockfiles
    comparison = []
    for (protocol, profile, payload, concurrency), metrics in sorted(samples.items()):
        for field in METRICS:
            comparison.append({
                "protocol": protocol,
                "profile": profile,
                "payload_size": payload,
                "concurrency": concurrency,
                "metric": field,
                **summarize(metrics["baseline", field], metrics["candidate", field]),
            })
    return metadata, observations, comparison


def md_cell(value):
    return escape(str(value)).replace("|", "&#124;").replace("\n", " ").replace("\r", " ")


def number(value):
    if value is None:
        return "—"
    return f"{value:,.2f}".removesuffix(".00")


def markdown(manifest, metadata, rows):
    lines = ["## MorphRPC Performance Lab", ""]
    if manifest["runner"]["kind"] == "github-hosted":
        lines += ["**Advisory.** Shared GitHub-hosted runner timings are diagnostic and carry no merge threshold.", ""]
    for side, sha in manifest["revisions"].items():
        lines.append(f"- {side.capitalize()} revision `{sha}`")
    lines += [
        f"- Host {md_cell(metadata['os'])}, {md_cell(metadata['cpu'])}",
        f"- Compiler {md_cell(metadata['build_rustc'])}",
        f"- Workload SHA-256 `{manifest['config_sha256']}`",
        "",
    ]
    lockfiles = metadata["cargo_lock_sha256"]
    if "baseline" in lockfiles and lockfiles["baseline"] != lockfiles["candidate"]:
        lines += [
            "**Dependency change.** Cargo.lock differs between revisions. Review dependency changes "
            "when interpreting performance differences.",
            "",
        ]
    if manifest["comparison_unavailable"]:
        lines += [manifest["comparison_unavailable"], "The candidate completed a standalone benchmark.", ""]
    else:
        lines += [
            f"{len(manifest['orders'])} paired blocks, alternating baseline-first and candidate-first.",
            "Timing and resource cells show median ± sample standard deviation across blocks.",
            "The change is the median paired percentage difference (candidate relative to baseline).",
            "Intervals have at least 95% nominal coverage for independent, identically distributed continuous blocks and are per metric.",
            "A dash marks an unavailable value. Intervals require at least six valid pairs.",
            "",
        ]
    lines += [
        "| Protocol | Profile | Data (B) | Connections | Metric | Baseline | Candidate | Paired change [interval] |",
        "| --- | --- | ---: | ---: | --- | ---: | ---: | ---: |",
    ]
    for row in rows:
        field = row["metric"]
        if not (row["baseline_n"] or row["candidate_n"]):
            continue
        cells = [
            row["protocol"], row["profile"], row["payload_size"],
            row["concurrency"], METRICS[field],
        ]
        for side in ("baseline", "candidate"):
            value = number(row[f"{side}_median"])
            sd = row[f"{side}_sd"]
            if sd is not None and field not in SIZES.keys() - {"server_rss_bytes", "cpu_usage"}:
                value += f" ± {number(sd)}"
            cells.append(value)
        delta = row["paired_median_delta_pct"]
        low, high = row["delta_ci_lower_pct"], row["delta_ci_upper_pct"]
        change = f"{delta:+.2f}%" if delta is not None else "—"
        if low is not None and field in TIMING.keys() | OPTIONAL_TIMING.keys():
            change += f" [{low:+.2f}%, {high:+.2f}%]"
        cells.append(change)
        lines.append("| " + " | ".join(map(md_cell, cells)) + " |")
    return "\n".join(lines) + "\n"


def generate(out):
    manifest = read_json(out / "manifest.json")
    metadata, observations, comparison = load_runs(out, manifest)
    write_json(out / "comparison.json", {"manifest": manifest, "metadata": metadata, "rows": comparison})
    write_csv(out / "comparison.csv", comparison)
    write_csv(out / "observations.csv", observations)
    (out / "summary.md").write_text(markdown(manifest, metadata, comparison), encoding="utf-8")
