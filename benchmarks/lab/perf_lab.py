"""Build and compare two committed MorphRPC revisions on one host."""

import argparse
from contextlib import ExitStack
import hashlib
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile

from perf_report import generate, write_json


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def revision(ref):
    return git("rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}")


def harness(sha):
    # Include all timed workload code, including generated service inputs.
    return git(
        "ls-tree", "-r", sha, "--", "benchmarks/src", "benchmarks/proto",
        "benchmarks/Cargo.toml", "benchmarks/build.rs",
    )


def run(command, cwd, env, log, timeout):
    print(log, flush=True)
    with log.open("w", encoding="utf-8") as stream:
        with subprocess.Popen(
            command, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT,
            start_new_session=True,
        ) as process:
            try:
                if process.wait(timeout=timeout):
                    raise ValueError(f"command failed; see {log}")
            except (KeyboardInterrupt, subprocess.TimeoutExpired):
                # The harness launches server children. End the whole process group.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                raise ValueError(f"command interrupted or timed out; see {log}") from None


def build(side, sha, scratch, out, env, stack):
    worktree = scratch / side
    target = scratch / f"{side}-target"
    subprocess.run(["git", "worktree", "add", "--detach", worktree, sha], check=True)
    stack.callback(subprocess.run, ["git", "worktree", "remove", "--force", worktree], check=True)
    run(
        ["cargo", "build", "--release", "--locked", "-p", "morphrpc-bench", "--target-dir", target],
        worktree, env, out / f"{side}-build.log", timeout=1200,
    )
    return worktree, target / "release" / "morphrpc-bench"


def compare(args):
    revisions = {"baseline": revision(args.baseline), "candidate": revision(args.candidate)}
    baseline_harness = harness(revisions["baseline"])
    candidate_harness = harness(revisions["candidate"])
    if not candidate_harness:
        raise ValueError("the candidate has no benchmark harness")
    unavailable = None
    if not baseline_harness:
        unavailable = "The baseline revision has no benchmark harness."
    elif baseline_harness != candidate_harness:
        unavailable = "The benchmark harness differs between revisions."
    if unavailable and not args.allow_unavailable_baseline:
        raise ValueError(unavailable)

    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    if any(out.iterdir()):
        raise ValueError(f"output directory must be empty: {out}")
    config = out / "config.toml"
    shutil.copyfile(args.config, config)
    toolchain = args.toolchain or subprocess.check_output(
        ["rustup", "show", "active-toolchain"], text=True,
    ).split()[0]
    env = {**os.environ, "RUSTUP_TOOLCHAIN": toolchain}
    compiler = subprocess.check_output(["rustc", "--version"], env=env, text=True).strip()
    sides = ["candidate"] if unavailable else ["baseline", "candidate"]
    # Alternate which revision goes first in consecutive pairs (AB, BA, AB, BA).
    orders = [sides if i % 2 == 0 else sides[::-1] for i in range(1 if unavailable else args.pairs)]
    manifest = {
        "schema_version": 1,
        "revisions": revisions,
        "toolchain": toolchain,
        "compiler": compiler,
        "build_environment": {
            key: env[key] for key in (
                "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_PROFILE_RELEASE_DEBUG",
            ) if key in env
        },
        "runner": {
            "kind": "github-hosted" if os.environ.get("GITHUB_ACTIONS") == "true" else "local",
            "image_os": os.environ.get("ImageOS"),
            "image_version": os.environ.get("ImageVersion"),
        },
        "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
        "harness_sha256": {
            "baseline": hashlib.sha256(baseline_harness.encode()).hexdigest() if baseline_harness else None,
            "candidate": hashlib.sha256(candidate_harness.encode()).hexdigest(),
        },
        "comparison_unavailable": unavailable,
        "orders": orders,
    }
    write_json(out / "manifest.json", manifest)

    with tempfile.TemporaryDirectory(prefix="morphrpc-lab-") as temp, ExitStack() as stack:
        scratch = Path(temp)
        builds = {side: build(side, revisions[side], scratch, out, env, stack) for side in sides}
        for pair, order in enumerate(orders, start=1):
            for side in order:
                worktree, binary = builds[side]
                destination = out / f"pair-{pair:02d}" / side
                destination.mkdir(parents=True)
                run(
                    [binary, "run", config, destination], worktree, env,
                    destination / "run.log", timeout=args.run_timeout_seconds,
                )
    generate(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    run_parser = commands.add_parser("compare", help="build, run, and report")
    run_parser.add_argument("baseline")
    run_parser.add_argument("candidate")
    run_parser.add_argument("--config", type=Path, required=True)
    run_parser.add_argument("--output", type=Path, required=True)
    run_parser.add_argument("--pairs", type=int, default=8)
    run_parser.add_argument("--toolchain", help="Rustup toolchain, defaults to the current checkout's")
    run_parser.add_argument("--run-timeout-seconds", type=int, default=1800, help="limit per benchmark invocation")
    run_parser.add_argument("--allow-unavailable-baseline", action="store_true")
    report_parser = commands.add_parser("report", help="regenerate reports from existing raw data")
    report_parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "compare":
            if args.pairs < 2 or args.pairs % 2:
                parser.error("--pairs must be an even integer of at least 2")
            if args.run_timeout_seconds < 1:
                parser.error("--run-timeout-seconds must be positive")
            compare(args)
        else:
            generate(args.output)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Performance Lab: {error}\n")


if __name__ == "__main__":
    main()
