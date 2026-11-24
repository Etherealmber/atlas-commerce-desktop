#!/usr/bin/env python3
"""Build the French GitHub release body and the short in-app update note.

Port of the atlas-commerce-android release notes generator: when the
release run supplies no manual notes, the body is summarised from the
commits between the previous version tag and HEAD.

Usage:
    python3 scripts/release-notes.py VERSION [--custom NOTES.txt] \
        --out-md RELEASE_NOTES.md --out-note IN_APP_NOTE.txt
"""

from __future__ import annotations

import argparse
import re
import subprocess
import pathlib
import os
from urllib.parse import quote

NOISE_RES = [
    r"^ci:\s*build log",
    r"^ci:\s*android 14 smoke log",
    r"^chore\(release\):\s*set version",
    r"^update readme\.md$",
    r"^merge (branch|pull request)",
    r"^co-authored-by:",
]

# Conventional-commit prefixes stripped from the subject line.
PREFIX_RE = re.compile(
    r"^(feat|fix|perf|refactor|chore|ci|ui|ux|security|docs|style|test)(\([^)]+\))?:\s*",
    re.IGNORECASE,
)
PR_RE = re.compile(r"\s*\(#\d+\)\s*$")


def sh(*args: str) -> str:
    try:
        return subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL).strip()
    except Exception:
        return ""


def norm_version(value: str) -> str:
    return re.sub(r"^[vV]", "", value.strip())


def is_bare_version(text: str) -> bool:
    return bool(re.fullmatch(r"[vV]?\d+\.\d+(?:\.\d+)?(?:\s*\(build\s*\d+\))?", text.strip()))


def clean_line(text: str) -> str:
    cleaned = re.sub(r"`([^`]+)`", r"\1", text)
    cleaned = re.sub(r"\*\*([^*]+)\*\*", r"\1", cleaned)
    cleaned = re.sub(r"\s+", " ", cleaned).strip().strip("-*• ").strip()
    return cleaned


def is_noise(text: str) -> bool:
    return any(re.search(pattern, text, re.IGNORECASE) for pattern in NOISE_RES)


def accept_item(target: list[str], seen: set[str], text: str) -> None:
    line = clean_line(text)
    if not line or len(line) < 6 or is_bare_version(line) or line.endswith(":"):
        return
    # Checked before and after conventional-prefix stripping, since entries
    # such as "ci: build log …" lose their prefix in the cleaned line.
    if is_noise(text) or is_noise(line):
        return
    key = re.sub(r"[^a-z0-9]+", "", line.lower())[:72]
    if not key or key in seen:
        return
    seen.add(key)
    target.append(line)


def previous_tag(version: str) -> str | None:
    for tag in sh("git", "tag", "--sort=-creatordate").splitlines():
        tag = tag.strip()
        if not tag or norm_version(tag) == norm_version(version):
            continue
        if subprocess.run(["git", "merge-base", "--is-ancestor", tag, "HEAD"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0:
            continue
        if (
            subprocess.run(
                ["git", "rev-parse", "--verify", f"{tag}^{{commit}}"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            ).returncode
            == 0
        ):
            return tag
    return None


def commit_items(version: str) -> list[str]:
    prev = previous_tag(version)
    rev_range = f"{prev}..HEAD" if prev else "HEAD~8..HEAD"
    raw_log = sh("git", "log", rev_range, "--no-merges", "--format=%H%x1f%s%x1f%b%x1e")
    if not raw_log:
        raw_log = sh("git", "log", "-n", "5", "--no-merges", "--format=%H%x1f%s%x1f%b%x1e")

    items: list[str] = []
    seen: set[str] = set()
    for entry in raw_log.split("\x1e"):
        entry = entry.strip()
        if not entry or "\x1f" not in entry:
            continue
        parts = entry.split("\x1f")
        raw_subject = (parts[1] if len(parts) > 1 else "").strip()
        if is_noise(raw_subject):
            continue
        subject = PREFIX_RE.sub("", raw_subject)
        subject = PR_RE.sub("", subject).strip()
        if subject:
            subject = subject[0].upper() + subject[1:]
        accept_item(items, seen, subject)
    return items


def category(subject: str) -> str:
    """Use only explicit conventional prefixes; never invent feature claims."""
    match = re.match(r"^(feat|fix|perf|security)(?:\([^)]+\))?!?:", subject, re.I)
    return {"feat": "Nouveautés", "fix": "Corrections", "perf": "Améliorations", "security": "Sécurité"}.get(match.group(1).lower(), "Autres changements") if match else "Autres changements"


def change_groups(version: str) -> dict[str, list[str]]:
    prev = previous_tag(version)
    rev_range = f"{prev}..HEAD" if prev else "HEAD"
    raw = sh("git", "log", rev_range, "--no-merges", "--format=%s%x1e")
    groups: dict[str, list[str]] = {}
    seen: set[str] = set()
    for raw_subject in raw.split("\x1e"):
        raw_subject = raw_subject.strip()
        if is_noise(raw_subject):
            continue
        item = PR_RE.sub("", PREFIX_RE.sub("", raw_subject)).strip()
        if item:
            item = item[0].upper() + item[1:]
        accepted: list[str] = []
        accept_item(accepted, seen, item)
        if accepted:
            groups.setdefault(category(raw_subject), []).extend(accepted)
    return groups


def render_body(version: str, custom: str, repo: str, asset_dir: pathlib.Path | None) -> str:
    tag = f"v{norm_version(version)}"
    # Repository must be owner/name; do not put untrusted strings in links.
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("invalid repository")
    base = f"https://github.com/{repo}"
    lines = [f"## Atlas Commerce Desktop `{tag}`", ""]
    if custom:
        lines += [custom, ""]
    else:
        lines += [f"Voici les changements inclus dans Atlas Commerce Desktop {tag}.", ""]
    # List only assets actually produced for this draft release. Do not guess
    # filenames or create links to optional formats that did not build.
    assets = sorted(p.name for p in asset_dir.iterdir() if p.is_file()) if asset_dir and asset_dir.is_dir() else []
    downloadable = [n for n in assets if n.startswith("atlas-commerce-desktop-") and n.endswith((".dmg", ".exe", ".msi", ".zip", ".tar.gz", ".AppImage")) and not n.endswith("-source.tar.gz")]
    if downloadable:
        links = [f"[{name}]({base}/releases/download/{quote(tag, safe='')}/{quote(name, safe='')})" for name in downloadable]
        lines += ["**Téléchargements :** " + " · ".join(links), ""]
    if not custom:
        groups = change_groups(version)
        for name in ("Nouveautés", "Corrections", "Améliorations", "Sécurité", "Autres changements"):
            if groups.get(name):
                lines += [f"**{name}**", ""]
                lines += ["- " + line for line in groups[name]]
                lines.append("")
        if not groups:
            lines += ["Aucun changement descriptif disponible dans l’historique.", ""]
    prev = previous_tag(version)
    if prev:
        lines += [f"**Changelog complet :** [{prev}...{tag}]({base}/compare/{quote(prev, safe='')}...{quote(tag, safe='')})", ""]
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    parser.add_argument("--custom", help="optional file with hand-written notes")
    parser.add_argument("--out-md", required=True)
    parser.add_argument("--out-note", required=True)
    parser.add_argument("--asset-dir", type=pathlib.Path, help="directory of built release assets")
    parser.add_argument("--repo", default=os.environ.get("RELEASE_REPO", "Etherealmber/atlas-commerce-desktop"))
    args = parser.parse_args()
    version = norm_version(args.version)
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?", version):
        parser.error("invalid release version")
    tag = f"v{version}"
    custom = pathlib.Path(args.custom).read_text(encoding="utf-8").strip() if args.custom else ""
    if is_bare_version(custom):
        custom = ""
    note = custom.splitlines()[0].strip() if custom else f"Mise à jour {tag} — améliorations et correctifs."
    if len(note) > 96:
        note = note[:93].rstrip() + "…"
    body = render_body(version, custom, args.repo, args.asset_dir)
    pathlib.Path(args.out_md).write_text(body, encoding="utf-8")
    pathlib.Path(args.out_note).write_text(note, encoding="utf-8")
    print(body)


if __name__ == "__main__":
    main()
