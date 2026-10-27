#!/usr/bin/env python3
"""Render tasks/tasks.yaml to markdown and create the GitHub labels, milestones and issues.

Usage:
  python3 scripts/create_issues.py --render-only                 # write tasks/T-*.md and tasks/README.md
  python3 scripts/create_issues.py --repo OWNER/REPO --dry-run   # show what would be created
  python3 scripts/create_issues.py --repo OWNER/REPO             # create everything (idempotent)

Requires PyYAML (pip install pyyaml) and, except for --render-only, an authenticated
GitHub CLI (gh auth login). Created issue numbers are stored in tasks/.issue-map.json, so
re-running only creates what is missing. Each task's `depends` list becomes GitHub's
native "blocked by" relationships, so GitHub (not the issue text) tracks what is ready.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover
    sys.exit("PyYAML is required: pip install pyyaml")

ROOT = Path(__file__).resolve().parent.parent
TASKS = ROOT / "tasks" / "tasks.yaml"
MAP_FILE = ROOT / "tasks" / ".issue-map.json"

LABEL_COLORS = {
    "area:": "1d76db",
    "determinism": "b60205",
    "ci": "5319e7",
    "post-1.0": "c5def5",
}
EXTRA_LABELS = {
    "task": ("0e8a16", "One PR-sized unit of engine work"),
    "budget-exempt": ("fbca04", "Maintainer only: exempt this PR from the 500-line budget"),
    "claude-review": ("7057ff", "Request another Claude review (Sonnet); removed automatically"),
    "deep-review": ("5319e7", "Claude reviews this PR with Opus"),
}


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")[:60]


def render(task: dict, local: bool = False) -> str:
    """Issue body. Local copies also list blockers; on GitHub they are native relationships."""
    header = [f"**Task {task['id']}** · {task['milestone']}"]
    if task["milestone"] != "Post-1.0":
        header[0] += f" · Est. core lines: ~{task['budget']} / 500"
    if local:
        header.append(f"**Blocked by:** {', '.join(task.get('depends', [])) or 'nothing'}")
    out = list(header)
    if task["milestone"] == "Post-1.0":
        out += ["", "## Goal", "", task["goal"], ""]
        out += ["_Do not start. Liam will split this into engine-sized tasks after 1.0._"]
        return "\n".join(out) + "\n"
    out += [
        "",
        "## Goal",
        "",
        task["goal"],
        "",
        "## Design references",
        "",
    ]
    out += [f"- `docs/design/strata-engine.md` {d}" for d in task.get("doc", [])] or ["- None"]
    out += ["", "## Scope", ""] + [f"- {s}" for s in task.get("scope", [])]
    out += ["", "## Acceptance criteria", ""] + [f"- [ ] {a}" for a in task.get("acceptance", [])]
    out += ["", "## Tests", ""] + [f"- {t}" for t in task.get("tests", [])]
    if task.get("out_of_scope"):
        out += ["", "## Out of scope", ""] + [f"- {o}" for o in task["out_of_scope"]]
    out += [
        "",
        "## Definition of done",
        "",
        "- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget",
        "- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue",
        "- [ ] Every acceptance criterion has a test",
        "- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`",
        "",
        "_Implementing agent: read `CLAUDE.md` first._",
    ]
    return "\n".join(out) + "\n"


def write_markdown(data: dict) -> None:
    tdir = ROOT / "tasks"
    for old in tdir.glob("T-*.md"):
        old.unlink()
    index = ["# Engine tasks", "", "Generated from `tasks.yaml` by `scripts/create_issues.py --render-only`.", ""]
    current = None
    for task in data["tasks"]:
        name = f"{task['id']}-{slug(task['title'])}.md"
        (tdir / name).write_text(f"# {task['id']}: {task['title']}\n\n" + render(task, local=True), encoding="utf-8")
        if task["milestone"] != current:
            current = task["milestone"]
            desc = next(m["description"] for m in data["milestones"] if m["title"] == current)
            index += ["", f"## {current}", "", desc, ""]
        deps = ", ".join(task.get("depends", [])) or "—"
        index.append(f"- [{task['id']}]({name}) {task['title']} · ~{task['budget']} lines · blocked by {deps}")
    (tdir / "README.md").write_text("\n".join(index) + "\n", encoding="utf-8")
    print(f"Wrote {len(data['tasks'])} task files and tasks/README.md")


class GH:
    def __init__(self, repo: str, dry_run: bool) -> None:
        self.repo = repo
        self.dry = dry_run

    def run(self, args: list[str], capture: bool = True) -> str:
        cmd = ["gh", *args]
        if self.dry:
            print("DRY:", " ".join(a if " " not in a else repr(a) for a in cmd))
            return ""
        res = subprocess.run(cmd, check=True, capture_output=capture, text=True)
        return res.stdout if capture else ""

    def api_json(self, path: str) -> list:
        if self.dry:
            return []
        out = subprocess.run(["gh", "api", "--paginate", path], check=True, capture_output=True, text=True).stdout
        # --paginate concatenates JSON arrays; normalize "][" boundaries.
        return json.loads("[" + out.strip()[1:-1].replace("][", ",") + "]") if out.strip() else []


def label_color(name: str) -> str:
    for prefix, color in LABEL_COLORS.items():
        if name.startswith(prefix):
            return color
    return "ededed"


def create_all(data: dict, gh: GH) -> None:
    labels = set(EXTRA_LABELS)
    for t in data["tasks"]:
        labels.update(t.get("labels", []))
    for name in sorted(labels):
        color, desc = EXTRA_LABELS.get(name, (label_color(name), ""))
        gh.run(["label", "create", name, "--repo", gh.repo, "--color", color, "--description", desc, "--force"])

    existing_ms = {m["title"] for m in gh.api_json(f"repos/{gh.repo}/milestones?state=all&per_page=100")}
    for m in data["milestones"]:
        if m["title"] not in existing_ms:
            gh.run(["api", f"repos/{gh.repo}/milestones", "--method", "POST", "-f", f"title={m['title']}", "-f", f"description={m['description']}"])

    id_map: dict[str, int] = json.loads(MAP_FILE.read_text()) if MAP_FILE.exists() else {}
    if not gh.dry:
        listed = json.loads(gh.run(["issue", "list", "--repo", gh.repo, "--state", "all", "--limit", "1000", "--json", "number,title"]))
        for issue in listed:
            m = re.match(r"(T-\d{3}):", issue["title"])
            if m and m.group(1) not in id_map:
                id_map[m.group(1)] = issue["number"]

    for t in data["tasks"]:
        if t["id"] in id_map:
            continue
        title = f"{t['id']}: {t['title']}"
        with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8") as fh:
            fh.write(render(t))
            body_path = fh.name
        args = ["issue", "create", "--repo", gh.repo, "--title", title, "--body-file", body_path, "--milestone", t["milestone"], "--label", "task"]
        for lab in t.get("labels", []):
            args += ["--label", lab]
        url = gh.run(args).strip()
        if gh.dry:
            id_map[t["id"]] = 0
            continue
        id_map[t["id"]] = int(url.rstrip("/").split("/")[-1])
        MAP_FILE.write_text(json.dumps(id_map, indent=2, sort_keys=True))
        print(f"Created {title} -> #{id_map[t['id']]}")

    link_dependencies(data, id_map, gh)


def link_dependencies(data: dict, id_map: dict[str, int], gh: GH) -> None:
    """Create GitHub "blocked by" relationships from each task's `depends` list (idempotent)."""
    issue_ids: dict[int, int] = {}

    def database_id(number: int) -> int:
        # The dependencies API takes the blocking issue's database id, not its number.
        if number not in issue_ids:
            issue_ids[number] = int(gh.run(["api", f"repos/{gh.repo}/issues/{number}", "--jq", ".id"]).strip())
        return issue_ids[number]

    added = 0
    for t in data["tasks"]:
        deps = t.get("depends", [])
        if not deps:
            continue
        if gh.dry:
            for d in deps:
                print(f"DRY: {t['id']} blocked by {d}")
            continue
        number = id_map[t["id"]]
        existing = {
            int(n)
            for n in gh.run(["api", f"repos/{gh.repo}/issues/{number}/dependencies/blocked_by", "--jq", ".[].number"]).split()
        }
        for d in deps:
            if id_map[d] in existing:
                continue
            gh.run([
                "api", f"repos/{gh.repo}/issues/{number}/dependencies/blocked_by",
                "--method", "POST", "-F", f"issue_id={database_id(id_map[d])}",
            ])
            added += 1
            time.sleep(0.5)  # the endpoint is subject to secondary rate limits
    print(f"Added {added} blocked-by relationships." if not gh.dry else "DRY: done")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", help="OWNER/REPO, for example BemusedVermin/achlydesa-strata")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--render-only", action="store_true")
    args = ap.parse_args()

    data = yaml.safe_load(TASKS.read_text(encoding="utf-8"))
    ids = [t["id"] for t in data["tasks"]]
    for t in data["tasks"]:
        for d in t.get("depends", []):
            if d not in ids or ids.index(d) >= ids.index(t["id"]):
                sys.exit(f"{t['id']} depends on {d}, which is missing or not earlier in the list")
        if t["milestone"] not in {m["title"] for m in data["milestones"]}:
            sys.exit(f"{t['id']} has unknown milestone {t['milestone']}")

    if args.render_only:
        write_markdown(data)
        return 0
    if not args.repo:
        ap.error("--repo is required unless --render-only")
    create_all(data, GH(args.repo, args.dry_run))
    return 0


if __name__ == "__main__":
    sys.exit(main())