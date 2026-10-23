#!/usr/bin/env python3
"""Bump the workspace version in the root Cargo.toml.

The desktop workspace keeps a single version: crates inherit it through
`version.workspace = true`, and the internal path dependencies in
`[workspace.dependencies]` carry the same number. There is no committed
Cargo.lock (it is regenerated in CI), so this manifest is the only file.

Usage: bump-workspace-version.py <new-version>

Exit codes:
    0  manifest updated
    2  manifest already at the requested version (nothing to do)
    1  invalid input or unexpected manifest shape
"""

import re
import sys
from pathlib import Path

SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
MANIFEST = Path(__file__).resolve().parents[1] / "Cargo.toml"


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: bump-workspace-version.py <x.y.z>", file=sys.stderr)
        return 1
    new = sys.argv[1].strip()
    if not SEMVER.match(new):
        print(f"invalid stable semver version: {new!r}", file=sys.stderr)
        return 1

    text = MANIFEST.read_text(encoding="utf-8")
    package = re.search(
        r"(\[workspace\.package\][^\[]*?version\s*=\s*\")([^\"]+)(\")",
        text,
        flags=re.DOTALL,
    )
    if not package:
        print("could not locate [workspace.package] version", file=sys.stderr)
        return 1
    old = package.group(2)
    if old == new:
        print(f"already at {new}")
        return 2

    def bump_dep(line: re.Match) -> str:
        return f"{line.group(1)}{new}{line.group(2)}"

    updated = package.group(1) + new + package.group(3)
    text = text[: package.start()] + updated + text[package.end() :]

    # Internal path dependencies must advertise the same workspace version.
    dep_pattern = re.compile(
        r"(^(?:atlas-core|atlas-api|atlas-ui)\s*=\s*\{[^\n]*?version\s*=\s*\")"
        r"[^\"]+(\")",
        flags=re.MULTILINE,
    )
    dep_hits = len(dep_pattern.findall(text))
    if dep_hits != 3:
        print(
            f"expected 3 internal workspace dependencies, found {dep_hits}",
            file=sys.stderr,
        )
        return 1
    text = dep_pattern.sub(bump_dep, text)

    MANIFEST.write_text(text, encoding="utf-8")
    print(f"bumped workspace {old} -> {new}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
