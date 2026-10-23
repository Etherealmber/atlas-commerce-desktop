#!/usr/bin/env python3
"""CI failure bot: turn a failed workflow run into a tracked GitHub issue.

Triggered by .github/workflows/ci-bot.yml after a CI / Packaging / Release
run completes with conclusion=failure. The script downloads the failed
logs, extracts the relevant error windows, reads the implicated source
files straight from the checked-out revision, attaches fix guidance and
files an issue (or comments on the existing one for the same revision).

Design notes:
  * all parsing lives in pure functions so it can be unit-tested without
    GitHub; only `collect()` and the issue helpers shell out to `gh`;
  * the script never exits non-zero: a reporting failure must not create
    a second failing check.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")
# crates/ui/src/screens/auth.rs:287, packaging/windows/atlas-commerce-desktop.iss:40,
# .github/workflows/release.yml:222, plus cargo's " --> path:line:col".
CODE_RE = re.compile(
    r"((?:crates|scripts|packaging|\.github)/[A-Za-z0-9_./-]+\."
    r"(?:rs|py|sh|yml|yaml|toml|iss|ps1|md)):(\d+)"
)
ERROR_PATTERNS = [
    re.compile(r"error(?:\[E\d+\])?:"),
    re.compile(r"panicked at"),
    re.compile(r"test result: FAILED"),
    re.compile(r"^failures:", re.MULTILINE),
    re.compile(r"bad substitution"),
    re.compile(r"Exception"),
    re.compile(r"::error::"),
    re.compile(r"Process completed with exit code"),
    re.compile(r"exited with code"),
    re.compile(r"cannot find|mismatched types|borrow of moved value"),
    re.compile(r"ISCC|Inno Setup", re.IGNORECASE),
    re.compile(r"^\s*throw\b", re.MULTILINE),
    re.compile(r"command failed|failed with", re.IGNORECASE),
]
# Runner workflow-command bookkeeping never carries the actual error.
NOISE_RE = re.compile(r"^\s*(##\[(?:group|endgroup)\])?\s*$")

# Ordered signatures: the first matching guidance is shown first.
REMEDIATION: list[tuple[re.Pattern[str], str]] = [
    (
        re.compile(r"bad substitution", re.IGNORECASE),
        "Bash received a GitHub expression as a shell variable. Use the "
        "`$RUNNER_OS` / `$GITHUB_*` environment variables inside `run:` "
        "blocks; `${{ ... }}` expressions belong in `if:`/`with:` contexts.",
    ),
    (
        re.compile(r"VCRUNTIME|MSVCP\d+.*\.dll", re.IGNORECASE),
        "The Windows binary dynamically links the MSVC runtime. Keep "
        "`-C target-feature=+crt-static` for the `*-pc-windows-msvc` targets "
        "in `.cargo/config.toml` so the exe is standalone.",
    ),
    (
        re.compile(r"osv-scanner|RUSTSEC|vulnerability", re.IGNORECASE),
        "An advisory was matched. Either `cargo update -p <crate> --precise "
        "<patched>` or add a scoped, dated entry to `osv-scanner.toml` for an "
        "unmaintained transitive crate with a written reason.",
    ),
    (
        re.compile(r"clippy|-D warnings", re.IGNORECASE),
        "Reproduce locally with `cargo clippy --all-targets --all-features "
        "-- -D warnings` (and `--fix` for the mechanical lints).",
    ),
    (
        re.compile(r"rustfmt|fmt --check|Formatting", re.IGNORECASE),
        "Run `cargo fmt --all`; the quality job enforces `--check`.",
    ),
    (
        re.compile(r"test result: FAILED|panicked at|assertion", re.IGNORECASE),
        "A test panicked. Reproduce with `cargo test --workspace <name>` and "
        "inspect the assertion line shown in the snippet below.",
    ),
    (
        re.compile(r"iscc|Inno Setup|\.iss", re.IGNORECASE),
        "The universal Inno Setup compile failed. Check the "
        "`/DVersion=… /DBinaryX64=… /DBinaryArm64=…` arguments, that both "
        "release exes were downloaded and that `IsX64`/`IsARM64` resolve on "
        "the runner (Inno Setup 6.3+).",
    ),
    (
        re.compile(r"lipo|notar|codesign|Apple", re.IGNORECASE),
        "macOS packaging/signing failed. Without Apple secrets the bundle is "
        "ad-hoc signed; verify `bundle.sh` and the universal `lipo` inputs.",
    ),
    (
        re.compile(r"native-packages|AppImage", re.IGNORECASE),
        "The native-packages/AppImage step failed. Re-run with the captured "
        "job log and check `native-packages.yaml` and the Ruby gem version.",
    ),
    (
        re.compile(r"dumpbin|PE machine|--version", re.IGNORECASE),
        "The standalone-Windows gate failed. The release exe must import no "
        "MSVC runtime (`.cargo/config.toml` crt-static), match its target PE "
        "machine and answer `--version` with the workspace version.",
    ),
]

DEFAULT_GUIDANCE = (
    "Start from the first error block below (later lines are often cascade), "
    "reproduce it locally, then use `gh run view <id> --log-failed` for the "
    "full output. Re-run the failed jobs once the fix is pushed."
)

LABEL = "ci-bot"
MAX_LINES_PER_JOB = 80
MAX_SNIPPET_FILES = 4
SNIPPET_RADIUS = 8


@dataclass
class JobFailure:
    name: str
    headline: str
    excerpt: str
    guidance: list[str] = field(default_factory=list)
    code_refs: list[tuple[str, int]] = field(default_factory=list)


def strip_ansi(text: str) -> str:
    return ANSI_RE.sub("", text)


def extract_excerpt(log: str, limit: int = MAX_LINES_PER_JOB) -> tuple[str, str]:
    """Return (headline, context window) around the first error signatures."""
    lines = [
        line
        for line in strip_ansi(log).splitlines()
        if not NOISE_RE.match(line)
    ]
    hit_indexes = [
        i
        for i, line in enumerate(lines)
        if any(pattern.search(line) for pattern in ERROR_PATTERNS)
    ]
    if not hit_indexes:
        # Last resort: the final non-empty lines usually hold the failing
        # command's own message just before "Process completed with exit code".
        tail = "\n".join(lines[-limit:]) if lines else ""
        headline = next(
            (
                line.strip()[:180]
                for line in reversed(lines)
                if "Process completed" not in line
            ),
            "no structured error line found",
        )
        return headline, tail

    first = hit_indexes[0]
    start = max(0, first - 4)
    selected: list[int] = []
    for hit in hit_indexes:
        selected.extend(range(max(0, hit - 2), min(len(lines), hit + 7)))
    selected = sorted(set(selected))
    # Keep the window contiguous around the first hits, then cap it.
    window = [i for i in selected if start <= i <= start + limit]
    excerpt = "\n".join(lines[i] for i in window).strip()
    headline = lines[first].strip()[:180]
    return headline, excerpt


def find_guidance(text: str) -> list[str]:
    lowered = text
    tips = [tip for pattern, tip in REMEDIATION if pattern.search(lowered)]
    if not tips:
        tips = [DEFAULT_GUIDANCE]
    # De-duplicate while preserving order.
    seen: set[str] = set()
    return [tip for tip in tips if not (tip in seen or seen.add(tip))]


def find_code_refs(text: str) -> list[tuple[str, int]]:
    refs: list[tuple[str, int]] = []
    seen: set[tuple[str, int]] = set()
    for match in CODE_RE.finditer(strip_ansi(text)):
        ref = (match.group(1), int(match.group(2)))
        if ref not in seen:
            seen.add(ref)
            refs.append(ref)
    return refs


def read_snippets(refs: list[tuple[str, int]], root: Path) -> list[tuple[str, int, str]]:
    snippets: list[tuple[str, int, str]] = []
    for path_str, line_no in refs[:MAX_SNIPPET_FILES]:
        path = root / path_str
        if not path.is_file():
            continue
        try:
            lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            continue
        lo = max(0, line_no - SNIPPET_RADIUS - 1)
        hi = min(len(lines), line_no + SNIPPET_RADIUS)
        numbered = "\n".join(f"{i + 1:>4} | {lines[i]}" for i in range(lo, hi))
        snippets.append((path_str, line_no, numbered))
    return snippets


def parse_failed_jobs(
    run_json: dict,
    logs: str,
    annotations: dict[str, list[str]] | None = None,
) -> list[JobFailure]:
    failures: list[JobFailure] = []
    annotations = annotations or {}
    failed_jobs = [job for job in run_json.get("jobs", []) if job.get("conclusion") == "failure"]
    for job in failed_jobs:
        name = job["name"]
        # gh run view --log prefixes every line with "jobname\t<step>\t<line>".
        job_lines = [
            line.split("\t", 2)[2] if line.count("\t") >= 2 else line
            for line in logs.splitlines()
            if line.split("\t", 1)[0] == name
        ]
        job_log = "\n".join(job_lines)
        # When the log endpoint yields nothing (token scopes, archived logs),
        # the check-run annotations still carry the ::error:: command output.
        if not job_log.strip():
            job_log = "\n".join(annotations.get(name, []))
        headline, excerpt = extract_excerpt(job_log)
        failures.append(
            JobFailure(
                name=name,
                headline=headline,
                excerpt=excerpt,
                guidance=find_guidance(job_log),
                code_refs=find_code_refs(job_log),
            )
        )
    return failures


def gh_json(*args: str) -> dict:
    output = subprocess.check_output(["gh", *args], text=True)
    import json

    return json.loads(output)


def collect(run_id: str, repo: str) -> tuple[dict, str]:
    run_json = gh_json(
        "run", "view", run_id, "--repo", repo,
        "--json", "jobs,displayTitle,headSha,workflowName,url,event,headBranch",
    )
    logs = subprocess.run(
        ["gh", "run", "view", run_id, "--repo", repo, "--log", "--failed"],
        text=True,
        capture_output=True,
        check=False,
    ).stdout
    return run_json, logs


def fetch_annotations(repo: str, sha: str) -> dict[str, list[str]]:
    """Check-run annotations keyed by job name — the fallback error channel.

    Workflow-command annotations survive even when `gh run view --log`
    cannot stream the log archive, so they are the reliable minimum.
    """
    try:
        check_runs = gh_json("api", f"repos/{repo}/commits/{sha}/check-runs")
    except Exception:  # noqa: BLE001 - annotations are best-effort
        return {}
    result: dict[str, list[str]] = {}
    for check in check_runs.get("check_runs", []):
        if check.get("conclusion") != "failure":
            continue
        try:
            payload = subprocess.check_output(
                ["gh", "api", f"repos/{repo}/check-runs/{check['id']}/annotations"],
                text=True,
            )
            import json

            messages = [
                item.get("message", "").strip()
                for item in json.loads(payload)
                if item.get("message")
            ]
            if messages:
                result[check["name"]] = messages
        except Exception:  # noqa: BLE001 - one bad check-run must not stop the rest
            continue
    return result


def issue_body(run_json: dict, failures: list[JobFailure], snippets: list[tuple[str, int, str]]) -> str:
    sha7 = run_json["headSha"][:7]
    parts: list[str] = [
        f"<!-- ci-bot: workflow={run_json['workflowName']} sha={sha7} -->",
        f"## \U0001F916 CI failure — {run_json['workflowName']} @ `{sha7}`",
        "",
        "| Field | Value |",
        "| --- | --- |",
        f"| Run | [{run_json['displayTitle']}]({run_json['url']}) |",
        f"| Event / branch | {run_json['event']} / `{run_json.get('headBranch', '')}` |",
        "| Failed jobs | " + ", ".join("`" + f.name + "`" for f in failures) + " |",
        "",
    ]
    for failure in failures:
        parts.extend(
            [
                f"### \U0001F538 {failure.name}",
                "",
                f"**First error:** `{failure.headline}`",
                "",
                "```text",
                failure.excerpt[:6000],
                "```",
                "**How to fix:**",
                *[f"- {tip}" for tip in failure.guidance],
                "",
            ]
        )
    if snippets:
        parts.append("### \U0001F4D6 Code at the implicated lines")
        parts.append("")
        for path_str, line_no, snippet in snippets:
            parts.extend(
                [
                    f"<details><summary><code>{path_str}:{line_no}</code></summary>",
                    "",
                    "```rust",
                    snippet,
                    "```",
                    "",
                    "</details>",
                    "",
                ]
            )
    parts.extend(
        [
            "---",
            "_Filed automatically by the CI failure bot. It will comment here "
            "if the same revision fails again; a fixed new revision opens a new issue._",
        ]
    )
    body = "\n".join(parts)
    return body[:65000]


def ensure_label(repo: str) -> None:
    subprocess.run(
        ["gh", "label", "create", LABEL, "--repo", repo,
         "--description", "Issues opened by the CI failure bot", "--force"],
        check=False,
        capture_output=True,
    )


def file_or_comment(repo: str, run_json: dict, body: str) -> str:
    sha7 = run_json["headSha"][:7]
    marker = f"sha={sha7}"
    search = subprocess.run(
        ["gh", "issue", "list", "--repo", repo, "--label", LABEL, "--state", "open",
         "--search", marker, "--json", "number,body", "--limit", "20"],
        text=True,
        capture_output=True,
        check=False,
    )
    import json

    existing = []
    if search.returncode == 0 and search.stdout.strip():
        existing = json.loads(search.stdout)
    for issue in existing:
        if marker in issue.get("body", ""):
            number = issue["number"]
            subprocess.run(
                ["gh", "issue", "comment", str(number), "--repo", repo,
                 "--body", f"Re-detected on run [{run_json['displayTitle']}]({run_json['url']}).\n\n{body}"],
                check=False,
            )
            return f"commented on #{number}"

    title = f"[CI bot] {run_json['workflowName']} failed @ {sha7} — {run_json['displayTitle'][:80]}"
    result = subprocess.run(
        ["gh", "issue", "create", "--repo", repo, "--title", title,
         "--label", LABEL, "--body", body],
        text=True,
        capture_output=True,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else f"issue creation failed: {result.stderr[:200]}"


def main() -> int:
    try:
        run_id = os.environ["CI_BOT_RUN_ID"]
        repo = os.environ.get("CI_BOT_REPO", os.environ.get("GITHUB_REPOSITORY", ""))
        if not run_id or not repo:
            print("ci-bot: missing CI_BOT_RUN_ID/CI_BOT_REPO", file=sys.stderr)
            return 0

        run_json, logs = collect(run_id, repo)
        annotations = fetch_annotations(repo, run_json["headSha"])
        failures = parse_failed_jobs(run_json, logs, annotations)
        if not failures:
            print("ci-bot: no failed jobs reported, nothing to file")
            return 0

        refs: list[tuple[str, int]] = []
        for failure in failures:
            for ref in failure.code_refs:
                if ref not in refs:
                    refs.append(ref)
        root = Path(os.environ.get("GITHUB_WORKSPACE", os.getcwd()))
        snippets = read_snippets(refs, root)

        body = issue_body(run_json, failures, snippets)
        ensure_label(repo)
        outcome = file_or_comment(repo, run_json, body)
        print(f"ci-bot: {outcome}")
    except Exception as exc:  # noqa: BLE001 - the bot must never fail a check
        print(f"ci-bot: suppressed reporter error: {exc!r}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
