#!/usr/bin/env python3
"""Enforce the 500-line core-logic budget for a pull request.

Counts added lines that are neither blank nor comment-only in "core logic" files
(see CLAUDE.md for the exact rules), compared against the merge base with --base.
Fails if the total exceeds --limit, or if a counted file adds an inline
`#[cfg(test)] mod tests { ... }` block (tests belong in separate tests.rs files).

Set BUDGET_EXEMPT=true to report without failing (CI sets it from the
`budget-exempt` label, which only the maintainer applies).
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys

LUA_LOGIC_DIRS = ("rules", "abilities", "scripts", "lib")
TEST_ATTR = re.compile(r"^\s*#\[cfg\(test\)\]\s*$")
INLINE_MOD = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*\{")


def is_counted(path: str) -> bool:
    """Return True if additions to `path` count toward the budget."""
    p = path.replace("\\", "/")
    parts = p.split("/")
    if p.startswith("crates/") and p.endswith(".rs"):
        if len(parts) == 3 and parts[2] == "build.rs":
            return True
        if len(parts) < 4 or parts[2] != "src":
            return False  # tests/, benches/, examples/ ...
        name = parts[-1]
        if name == "tests.rs" or name.endswith("_tests.rs"):
            return False
        if "tests" in parts[3:-1]:
            return False
        return True
    if p.startswith("packs/") and p.endswith(".lua"):
        # packs/<pack>/<dir>/...lua ; top-level files (pack.lua, data*.lua) are not logic
        return len(parts) >= 4 and parts[2] in LUA_LOGIC_DIRS
    return False


class CommentTracker:
    """Tracks block comments well enough to skip comment-only lines."""

    def __init__(self, ext: str) -> None:
        self.ext = ext
        self.in_block = False

    def is_comment_or_blank(self, line: str) -> bool:
        s = line.strip()
        if not s:
            return True
        if self.ext == ".rs":
            if self.in_block:
                if "*/" in s:
                    self.in_block = False
                return True
            if s.startswith("/*"):
                if "*/" not in s[2:]:
                    self.in_block = True
                return True
            return s.startswith("//")
        # Lua
        if self.in_block:
            if "]]" in s or re.search(r"\]=+\]", s):
                self.in_block = False
            return True
        if re.match(r"--\[=*\[", s):
            if not ("]]" in s[4:] or re.search(r"\]=+\]", s[4:])):
                self.in_block = True
            return True
        return s.startswith("--")


def git_diff(base: str) -> str:
    cmd = ["git", "diff", "--unified=0", "--no-color", "--no-ext-diff", f"{base}...HEAD"]
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def analyse(diff: str) -> tuple[dict[str, int], list[str]]:
    counts: dict[str, int] = {}
    violations: list[str] = []
    path: str | None = None
    tracker: CommentTracker | None = None
    pending_cfg_test = False

    for raw in diff.splitlines():
        if raw.startswith("+++ "):
            target = raw[4:].strip()
            path = None if target == "/dev/null" else target[2:] if target.startswith("b/") else target
            if path is not None and is_counted(path):
                tracker = CommentTracker(os.path.splitext(path)[1])
                counts.setdefault(path, 0)
            else:
                tracker = None
            pending_cfg_test = False
            continue
        if raw.startswith("@@"):
            pending_cfg_test = False
            continue
        if tracker is None or path is None or not raw.startswith("+") or raw.startswith("+++"):
            continue
        line = raw[1:]
        if path.endswith(".rs"):
            if TEST_ATTR.match(line):
                pending_cfg_test = True
            elif line.strip():
                if pending_cfg_test and INLINE_MOD.match(line):
                    violations.append(
                        f"{path}: inline `#[cfg(test)] mod ... {{` block; move tests to a separate tests.rs file"
                    )
                pending_cfg_test = False
        if not tracker.is_comment_or_blank(line):
            counts[path] += 1
    return counts, violations


def write_summary(text: str) -> None:
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as fh:
            fh.write(text + "\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", default="origin/master", help="base ref to diff against (merge base is used)")
    ap.add_argument("--limit", type=int, default=500)
    args = ap.parse_args()

    counts, violations = analyse(git_diff(args.base))
    total = sum(counts.values())
    exempt = os.environ.get("BUDGET_EXEMPT", "").lower() == "true"

    rows = sorted(((n, p) for p, n in counts.items() if n), reverse=True)
    lines = ["### Core-logic line budget", "", "| Added lines | File |", "| ---: | --- |"]
    lines += [f"| {n} | `{p}` |" for n, p in rows] or ["| 0 | (no counted files changed) |"]
    lines += ["", f"**Total: {total} / {args.limit}**" + (" (budget-exempt)" if exempt else "")]
    if violations:
        lines += ["", "**Violations:**"] + [f"- {v}" for v in violations]
    report = "\n".join(lines)
    print(report)
    write_summary(report)

    if exempt:
        return 0
    if violations:
        print("\nFAIL: inline test modules are not allowed in counted files.", file=sys.stderr)
        return 1
    if total > args.limit:
        print(
            f"\nFAIL: {total} core-logic lines added; the limit is {args.limit}. "
            "Split the work into smaller PRs (see CLAUDE.md).",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
