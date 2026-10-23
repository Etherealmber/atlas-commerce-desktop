#!/usr/bin/env python3
"""CI failure bot: turn a failed workflow run into a tracked GitHub issue.

Triggered by .github/workflows/ci-bot.yml after a CI / Packaging / Release
run completes. On failure the script downloads the logs, extracts the error
windows, reads the implicated source files straight from the checked-out
revision, attaches fix guidance (plus an optional Groq AI diagnosis when the
GROQ_API_KEY secret is configured) and either:

  * comments on the existing issue for the same revision, or
  * comments on the open issue already tracking this branch / pull request,
    so repeated failing commits never open duplicate issues, or
  * opens a new issue.

On a successful run it closes the open bot issues for that branch / pull
request with a green-run comment.

Design notes:
  * all parsing and matching lives in pure functions so it can be unit-tested
    without GitHub (`python3 scripts/ci-bot-report.py --selftest`); only
    `collect()` and the issue helpers shell out to `gh`;
  * the script never exits non-zero: a reporting failure must not create a
    second failing check.
"""

from __future__ import annotations

import datetime
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
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
# The machine-readable marker stored in every issue the bot creates.
MARKER_RE = re.compile(r"<!-- ci-bot: (?P<inner>.*?) -->")
GROQ_ENDPOINT = "https://api.groq.com/openai/v1/chat/completions"
GROQ_MODEL = "llama-3.3-70b-versatile"

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
        re.compile(r"wix|\.msi|CNDL|LGHT", re.IGNORECASE),
        "The WiX MSI generator failed. It runs `packaging/windows/build-msi.py` "
        "on the release payload with the wix 5.0.2 dotnet tool; reproduce with "
        "`python packaging/windows/build-msi.py <payload> out.msi 0.1.0 amd64` "
        "and check the CNDL/LGHT compiler lines surfaced in the annotations.",
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


# ── Issue identity: markers, matching and triage ────────────────────────────


def first_pr_number(run_json: dict) -> str | None:
    """The pull request number associated with the run, when there is one."""
    for pr in run_json.get("pullRequests") or []:
        number = pr.get("number")
        if number is not None:
            return str(number)
    return None


def first_pr_url(run_json: dict, repo: str) -> str | None:
    for pr in run_json.get("pullRequests") or []:
        number = pr.get("number")
        if number is not None:
            return pr.get("url") or f"https://github.com/{repo}/pull/{number}"
    return None


PR_EVENTS = ("pull_request", "pull_request_target")


def pr_refs_for_event(event: str, refs: list[dict]) -> list[dict]:
    """Filter commits/pulls API output down to real PR-event membership.

    The same API answers for push runs (a merge commit belongs to its merged
    PR), but adopting that number on a push run would cross the threads, so
    non-PR events always get an empty list.
    """
    if event not in PR_EVENTS:
        return []
    return [
        {"number": pr.get("number"), "url": pr.get("html_url", "")}
        for pr in refs
        if pr.get("number") is not None
    ]


def parse_timestamp(value: str | None) -> datetime.datetime | None:
    if not value:
        return None
    try:
        return datetime.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def green_run_is_current(green_created: str | None, failure_committed: str | None) -> bool:
    """Whether a green run may close an issue whose last failure is known.

    False (keep the issue open) when either timestamp is missing or the green
    run started before the failure revision was committed: runs can finish
    out of order across concurrency groups, so an older green run must not
    close an issue tracking a newer still-failing revision.
    """
    green = parse_timestamp(green_created)
    failure = parse_timestamp(failure_committed)
    if green is None or failure is None:
        return False
    return green >= failure


def issue_marker(run_json: dict) -> str:
    """Build the machine-readable identity comment stored in every issue."""
    sha7 = run_json["headSha"][:7]
    branch = (run_json.get("headBranch") or "").replace(" ", "-")
    pr = first_pr_number(run_json) or "-"
    return (
        f"<!-- ci-bot: workflow={run_json['workflowName']} "
        f"sha={sha7} branch={branch} pr={pr} -->"
    )


def parse_marker(body: str) -> dict[str, str]:
    match = MARKER_RE.search(body or "")
    if not match:
        return {}
    fields: dict[str, str] = {}
    for token in match.group("inner").split():
        if "=" in token:
            key, value = token.split("=", 1)
            fields[key] = value
    return fields


def marker_matches(marker: dict[str, str], run_json: dict) -> str | None:
    """How strongly an existing issue matches a run: 'sha', 'thread' or None.

    'sha'    — the exact same revision failed again (same marker sha).
    'thread' — the same workflow on the same branch / pull request failed on a
               newer revision: keep one issue per line of work instead of
               opening a duplicate per commit.
    """
    if not marker or marker.get("workflow") != run_json["workflowName"]:
        return None
    sha7 = run_json["headSha"][:7]
    if marker.get("sha") == sha7:
        return "sha"
    pr = first_pr_number(run_json)
    marker_pr = marker.get("pr", "-")
    if pr is not None:
        # A run tied to a pull request only follows the issue of that same
        # PR: a matching branch name across different PRs (e.g. fork PRs
        # whose head branch is called "main") must never cross-link.
        return "thread" if marker_pr != "-" and marker_pr == pr else None
    # A run without a PR (plain branch push) only threads onto markers that
    # themselves carry no PR, on the same branch.
    branch = (run_json.get("headBranch") or "").replace(" ", "-")
    if marker_pr == "-" and branch and marker.get("branch") == branch:
        return "thread"
    return None


def gh_json(*args: str) -> dict:
    output = subprocess.check_output(["gh", *args], text=True)
    return json.loads(output)


def collect(run_id: str, repo: str) -> tuple[dict, str]:
    # `gh run view --json` has no pullRequests field: PR membership is
    # resolved separately through the commits API below. createdAt is needed
    # to ignore stale green runs (out-of-order completion).
    run_json = gh_json(
        "run", "view", run_id, "--repo", repo,
        "--json",
        "jobs,displayTitle,headSha,workflowName,url,event,headBranch,createdAt",
    )
    refs: list = []
    # Only pull-request events inherit PR membership. A push to the default
    # branch whose sha happens to be a merge commit must not adopt the merged
    # PR: push and merge-group runs thread by branch only.
    if (run_json.get("event") or "") in PR_EVENTS:
        try:
            refs = json.loads(
                subprocess.check_output(
                    ["gh", "api", f"repos/{repo}/commits/{run_json.get('headSha')}/pulls"],
                    text=True,
                )
            )
        except Exception:  # noqa: BLE001 - PR metadata is best-effort
            refs = []
    run_json["pullRequests"] = pr_refs_for_event(run_json.get("event") or "", refs)
    logs = subprocess.run(
        ["gh", "run", "view", run_id, "--repo", repo, "--log", "--failed"],
        text=True,
        capture_output=True,
        check=False,
    ).stdout
    return run_json, logs


def commit_date(repo: str, sha: str | None) -> str | None:
    """ISO commit timestamp for a (possibly short) sha, best-effort."""
    if not sha:
        return None
    try:
        payload = gh_json("api", f"repos/{repo}/commits/{sha}")
        return payload.get("commit", {}).get("committer", {}).get("date")
    except Exception:  # noqa: BLE001 - freshness check must stay best-effort
        return None


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


def groq_diagnosis(run_json: dict, failures: list[JobFailure]) -> str | None:
    """Ask Groq for a short French diagnosis when GROQ_API_KEY is configured.

    Any failure (no key, network, model) degrades to None: AI summaries are a
    nice-to-have and must never block issue filing.
    """
    key = os.environ.get("GROQ_API_KEY", "").strip()
    if not key or not failures:
        return None
    compact = "\n\n".join(
        f"### {failure.name}\nFirst error: {failure.headline}\n{failure.excerpt[:1800]}"
        for failure in failures[:4]
    )
    prompt = (
        "Tu analyses un échec de CI GitHub Actions pour une application Rust/egui. "
        "Donne un diagnostic concis en français : cause probable la plus spécifique, "
        "puis 2 ou 3 actions de correction ordonnées. Pas de hors-sujet, pas de markdown "
        "volumineux, max 180 mots.\n\n"
        f"Workflow : {run_json['workflowName']} ({run_json['event']})\n\n{compact}"
    )
    payload = {
        "model": GROQ_MODEL,
        "temperature": 0.2,
        "max_tokens": 500,
        "messages": [
            {
                "role": "system",
                "content": "Tu es un ingénieur CI/CD rigoureux. Réponds uniquement en français.",
            },
            {"role": "user", "content": prompt},
        ],
    }
    request = urllib.request.Request(
        GROQ_ENDPOINT,
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=25) as response:
            data = json.loads(response.read().decode("utf-8"))
        content = data["choices"][0]["message"]["content"].strip()
        return content or None
    except (urllib.error.URLError, KeyError, ValueError, TimeoutError) as error:
        print(f"ci-bot: Groq summary skipped ({error!r})", file=sys.stderr)
        return None


def issue_body(
    run_json: dict,
    failures: list[JobFailure],
    snippets: list[tuple[str, int, str]],
    repo: str,
) -> str:
    sha7 = run_json["headSha"][:7]
    parts: list[str] = [
        issue_marker(run_json),
        f"## \U0001F916 CI failure — {run_json['workflowName']} @ `{sha7}`",
        "",
        "| Field | Value |",
        "| --- | --- |",
        f"| Run | [{run_json['displayTitle']}]({run_json['url']}) |",
        f"| Event / branch | {run_json['event']} / `{run_json.get('headBranch', '')}` |",
    ]
    pr_url = first_pr_url(run_json, repo)
    if pr_url:
        parts.append(f"| Pull request | {pr_url} |")
    parts.extend(
        [
            "| Failed jobs | " + ", ".join("`" + f.name + "`" for f in failures) + " |",
            "",
        ]
    )
    ai = groq_diagnosis(run_json, failures)
    if ai:
        parts.extend(
            [
                "### \U0001F9E0 Diagnostic IA (Groq)",
                "",
                ai,
                "",
            ]
        )
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
            "_Filed automatically by the CI failure bot. New failing revisions on "
            "the same branch / pull request are commented here instead of opening a "
            "duplicate issue; a green run closes it._",
        ]
    )
    return "\n".join(parts)[:65000]


def ensure_label(repo: str) -> None:
    subprocess.run(
        ["gh", "label", "create", LABEL, "--repo", repo,
         "--description", "Issues opened by the CI failure bot", "--force"],
        check=False,
        capture_output=True,
    )


def open_label_issues(repo: str) -> list[dict]:
    """All open bot issues with their parsed markers (small, bounded list)."""
    result = subprocess.run(
        ["gh", "issue", "list", "--repo", repo, "--label", LABEL,
         "--state", "open", "--json", "number,title,body", "--limit", "100"],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return []
    issues = json.loads(result.stdout)
    for issue in issues:
        issue["marker"] = parse_marker(issue.get("body", ""))
    return issues


def triage(repo: str, run_json: dict, body: str) -> str:
    """Comment on an existing issue when one already tracks this failure.

    Same-revision failures and same-thread (branch / pull request) failures
    both reuse the existing issue; only genuinely new lines of work create
    one. Returns a human-readable outcome for the workflow log.
    """
    sha7 = run_json["headSha"][:7]
    for issue in open_label_issues(repo):
        kind = marker_matches(issue.get("marker", {}), run_json)
        if kind == "sha":
            number = issue["number"]
            comment = (
                f"Re-detected on run [{run_json['displayTitle']}]({run_json['url']}).\n\n{body}"
            )
            subprocess.run(
                ["gh", "issue", "comment", str(number), "--repo", repo, "--body", comment],
                check=False,
            )
            return f"commented on existing issue #{number} (same revision {sha7})"
    for issue in open_label_issues(repo):
        kind = marker_matches(issue.get("marker", {}), run_json)
        if kind == "thread":
            number = issue["number"]
            comment = (
                f"Still failing on `{sha7}` — run "
                f"[{run_json['displayTitle']}]({run_json['url']}).\n\n"
                "This issue already tracks the branch/pull request, so no new "
                f"issue was created. Latest diagnosis:\n\n{body}"
            )
            subprocess.run(
                ["gh", "issue", "comment", str(number), "--repo", repo, "--body", comment],
                check=False,
            )
            return f"commented on existing issue #{number} (already tracked, no duplicate)"
    title = f"[CI bot] {run_json['workflowName']} failed @ {sha7} — {run_json['displayTitle'][:80]}"
    result = subprocess.run(
        ["gh", "issue", "create", "--repo", repo, "--title", title,
         "--label", LABEL, "--body", body],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode == 0:
        return f"created {result.stdout.strip()}"
    return f"issue creation failed: {result.stderr[:200]}"


def resolve_green_runs(repo: str, run_json: dict) -> str:
    """Close the bot issues a successful run just fixed (same branch / PR)."""
    closed: list[int] = []
    for issue in open_label_issues(repo):
        kind = marker_matches(issue.get("marker", {}), run_json)
        if kind is None:
            continue
        number = issue["number"]
        # Out-of-order guard: the green run must be no older than the failure
        # revision recorded on the issue. Different concurrency groups let
        # runs complete in either order; a stale green run keeps the issue.
        marker_sha = issue.get("marker", {}).get("sha")
        failure_committed = commit_date(repo, marker_sha)
        if not green_run_is_current(run_json.get("createdAt"), failure_committed):
            print(
                f"ci-bot: keep #{number} open: green run {run_json.get('createdAt')} "
                f"is not newer than failure {marker_sha} ({failure_committed})"
            )
            continue
        sha7 = run_json["headSha"][:7]
        comment = (
            f"✅ Green again on `{sha7}` — run "
            f"[{run_json['displayTitle']}]({run_json['url']}) succeeded. "
            "Closing automatically."
        )
        subprocess.run(
            ["gh", "issue", "comment", str(number), "--repo", repo, "--body", comment],
            check=False,
            capture_output=True,
        )
        closed_result = subprocess.run(
            ["gh", "issue", "close", str(number), "--repo", repo,
             "--reason", "completed", "--comment",
             f"Fixed by the green run {run_json['url']}."],
            check=False,
            capture_output=True,
            text=True,
        )
        if closed_result.returncode == 0:
            closed.append(number)
    if closed:
        return f"closed fixed issues: {', '.join(f'#{n}' for n in closed)}"
    return "no open bot issue matched the green run"


def run_report(run_id: str, repo: str, conclusion: str) -> int:
    run_json, logs = collect(run_id, repo)
    if conclusion == "success":
        print(f"ci-bot: {resolve_green_runs(repo, run_json)}")
        return 0

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
    # The failing revision is checked out as read-only data under
    # CI_BOT_REVISION_DIR; the executed bot script itself always comes from
    # the trusted default-branch checkout (see ci-bot.yml).
    root = Path(os.environ.get("CI_BOT_REVISION_DIR") or
                os.environ.get("GITHUB_WORKSPACE", os.getcwd()))
    snippets = read_snippets(refs, root)

    body = issue_body(run_json, failures, snippets, repo)
    ensure_label(repo)
    outcome = triage(repo, run_json, body)
    print(f"ci-bot: {outcome}")
    return 0


def selftest() -> int:
    """Pure-function checks; no network and no `gh` calls."""
    run = {
        "workflowName": "CI",
        "headSha": "abc1234deadbeef",
        "headBranch": "arena/2f55b264-atlas-commerce-desktop",
        "event": "pull_request",
        "displayTitle": "feat: thing",
        "url": "https://example.invalid/run/1",
        "pullRequests": [{"number": 42, "url": "https://example.invalid/pr/42"}],
        "jobs": [],
    }
    marker = issue_marker(run)
    assert "workflow=CI" in marker and "sha=abc1234" in marker
    assert "pr=42" in marker and "branch=arena/2f55b264" in marker
    parsed = parse_marker(marker)
    assert parsed["workflow"] == "CI" and parsed["pr"] == "42"

    # Same revision matches at the strongest level.
    assert marker_matches(parsed, run) == "sha"

    # A newer revision on the same PR reuses the issue instead of duplicating.
    newer = dict(run, headSha="999888777")
    assert marker_matches(parsed, newer) == "thread"

    # A different workflow on the same branch must not match.
    other_workflow = dict(run, workflowName="Release", headSha="999888777")
    assert marker_matches(parsed, other_workflow) is None

    # A run on another branch with no PR does not match.
    main_run = dict(
        run,
        headSha="555444333",
        headBranch="main",
        pullRequests=[],
    )
    assert marker_matches(parsed, main_run) is None

    # A different pull request on an identically named branch must not match,
    # even though branch and workflow are identical.
    other_pr = dict(
        run,
        headSha="555444333",
        headBranch="arena/2f55b264-atlas-commerce-desktop",
        pullRequests=[{"number": 7, "url": "https://example.invalid/pr/7"}],
    )
    assert marker_matches(parsed, other_pr) is None

    # A plain branch-push run (no PR) cannot claim an issue filed for a PR.
    push_run = dict(run, headSha="555444333", pullRequests=[])
    assert marker_matches(parsed, push_run) is None

    # Two non-PR runs on the same branch do thread together.
    branch_marker = parse_marker(
        issue_marker(dict(run, headBranch="main", pullRequests=[]))
    )
    branch_run = {
        "workflowName": "CI",
        "headSha": "aaa111bbb222",
        "headBranch": "main",
        "event": "push",
        "displayTitle": "push",
        "url": "https://example.invalid/run/2",
        "pullRequests": [],
        "jobs": [],
    }
    assert marker_matches(branch_marker, branch_run) == "thread"
    # ... but a fork PR whose head branch is also called "main" does not.
    fork_pr = dict(branch_run, pullRequests=[{"number": 99, "url": "u"}])
    assert marker_matches(branch_marker, fork_pr) is None

    # Markers absent from free-form issue bodies parse to nothing.
    assert parse_marker("plain body, no marker") == {}

    # Excerpt extraction still finds the first cargo error and strips ANSI.
    log = "\x1b[31merror[E0433]: cannot find module `widgets`\x1b[0m\ncascade line"
    headline, excerpt = extract_excerpt(log)
    assert "E0433" in headline and "\x1b[31m" not in excerpt

    # Groq stays opt-in: without a key there is no call and no crash.
    os.environ.pop("GROQ_API_KEY", None)
    assert groq_diagnosis(run, []) is None

    # PR membership only attaches to pull-request events: a push whose sha is
    # a merge commit must not inherit the merged pull request's number.
    merged = [{"number": 42, "html_url": "https://example.invalid/pr/42"}]
    assert pr_refs_for_event("push", merged) == []
    assert pr_refs_for_event("merge_group", merged) == []
    pr_refs = pr_refs_for_event("pull_request", merged)
    assert pr_refs == [{"number": 42, "url": "https://example.invalid/pr/42"}]
    assert pr_refs_for_event("pull_request_target", merged)[0]["number"] == 42

    # Stale green runs cannot close an issue tracking a newer failure.
    newer_failure = "2026-10-24T10:00:00Z"
    older_green = "2026-10-24T09:00:00.000+00:00"
    current_green = "2026-10-24T11:00:00Z"
    assert green_run_is_current(older_green, newer_failure) is False
    assert green_run_is_current(current_green, newer_failure) is True
    # Missing timestamps fail closed: the issue stays open.
    assert green_run_is_current(None, newer_failure) is False
    assert green_run_is_current(current_green, None) is False

    print("ci-bot selftest: all checks passed")
    return 0


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--selftest":
        return selftest()
    try:
        run_id = os.environ["CI_BOT_RUN_ID"]
        repo = os.environ.get("CI_BOT_REPO", os.environ.get("GITHUB_REPOSITORY", ""))
        conclusion = os.environ.get("CI_BOT_CONCLUSION", "failure").strip().lower()
        if not run_id or not repo:
            print("ci-bot: missing CI_BOT_RUN_ID/CI_BOT_REPO", file=sys.stderr)
            return 0
        if conclusion not in {"failure", "success"}:
            print(f"ci-bot: conclusion '{conclusion}' needs no issue action")
            return 0
        return run_report(run_id, repo, conclusion)
    except Exception as exc:  # noqa: BLE001 - the bot must never fail a check
        print(f"ci-bot: suppressed reporter error: {exc!r}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
