# Bootstrapping achlydesa-strata

This kit turns the empty repo into one a Sonnet agent can work through issue by issue, with CI and an automated Claude review in front of every PR you look at.

## What's here

| Path | Purpose |
| --- | --- |
| `CLAUDE.md` | The agent's rulebook: workflow, the 500-line rule, determinism, layering. Claude Code loads it automatically. |
| `docs/design/strata-engine.md` | Repo copy of the engine design doc. Issues cite its sections. |
| `tasks/tasks.yaml` | 70 tasks (T-001 to T-070) across milestones M0–M7 plus a post-1.0 campaign placeholder. |
| `tasks/T-*.md`, `tasks/README.md` | The same tasks rendered as markdown, with an index. |
| `scripts/create_issues.py` | Creates labels, milestones and issues with `gh`, and turns each task's prerequisites into GitHub "blocked by" relationships. Idempotent. |
| `scripts/line_budget.py` | Counts added core-logic lines per PR; fails above 500 or on inline test modules. |
| `.github/workflows/ci.yml` | fmt + clippy, tests on Linux/macOS/Windows, cargo-deny. Skips Rust steps until T-001 adds the workspace. |
| `.github/workflows/line-budget.yml` | The 500-line check; re-runs when the `budget-exempt` label changes. |
| `.github/workflows/claude-review.yml` | Advisory Claude review on every non-draft PR to `master`: inline comments plus a verdict summary. |
| `.github/workflows/claude.yml` | Optional: comment `@claude ...` on an issue or PR to have Claude implement or fix. Only you and collaborators can trigger it. |
| `.github/pull_request_template.md`, `ISSUE_TEMPLATE/task.md`, `CODEOWNERS` | PR checklist, task template, you as owner of everything. |

## One-time setup

1. **Commit the kit.** Copy everything into the repo root (keep your README) and push straight to `master`:
   `git add . && git commit -m "Bootstrap: CI, Claude review, agent guide, design doc, tasks" && git push`
2. **Install the Claude GitHub app** on the repo: <https://github.com/apps/claude>.
3. **Add your Claude token.** Settings → Secrets and variables → Actions → New repository secret named `CLAUDE_CODE_OAUTH_TOKEN`, with the token you already have (from `claude setup-token`) as the value.
4. **Optional variables** (same page, Variables tab): `CLAUDE_REVIEW_MODEL` (default `claude-opus-5-5`) and `CLAUDE_IMPL_MODEL` (default `claude-sonnet-5-5`).
5. **Protect `master`** (Settings → Rules → Rulesets, or Branch protection):
   - Require a pull request before merging; block force pushes and deletions.
   - Require status checks: `fmt + clippy`, `test (ubuntu-latest)`, `test (macos-latest)`, `test (windows-latest)`, `cargo-deny`, `core logic <= 500 lines`. Checks only appear in the picker after they have run once, so add them after the first PR.
   - Leave the Claude review advisory, not required.
   - Approvals: GitHub won't let you approve a PR opened from your own account. If the agent runs locally under your account, keep required approvals at 0 and simply be the one who merges. If PRs come from the Claude app, you can require one code-owner approval.
6. **Create the issues:**
   ```
   pip install pyyaml
   gh auth login
   python3 scripts/create_issues.py --repo BemusedVermin/achlydesa-strata --dry-run
   python3 scripts/create_issues.py --repo BemusedVermin/achlydesa-strata
   git add tasks/.issue-map.json && git commit -m "Record issue numbers" && git push
   ```

## Running the agent

Pick any open issue with no open **Blocked by** entries in its sidebar; GitHub keeps that current as PRs merge and issues close. Unblocked tasks (for example T-002 and T-003 once T-001 closes) can run in parallel branches. The `@claude` workflow checks blockers itself and declines to start a blocked issue.

- **Locally with Claude Code:** in the repo, run `claude --model sonnet` and say: "Implement issue #N (T-0XX). Follow CLAUDE.md, then open the PR."
- **On GitHub:** comment on the issue: `@claude implement this issue following CLAUDE.md, then open a PR against master.`

Either way the PR gets CI, the line-budget check and the Claude review before you look. To ask the agent to address review comments, comment `@claude address the review comments` on the PR, or continue the local session.

## Your review loop

1. Wait for CI and the Claude review summary ("Ready for human review" or "Changes needed").
2. Review against the issue's acceptance criteria. The PR template lists which test covers each one.
3. Squash-merge. The issue closes from `Closes #N`.
4. If a task turns out too big, the agent will propose a split on the issue; add the follow-up with the same format in `tasks.yaml` and re-run `create_issues.py`.

## Things only you do (deliberately not issues)

- Units, factions and sprite art for `strata_std` (M7).
- Approving Bevy version bumps (T-047 pins one; upgrades touch only `strata_client`).
- Keeping `docs/design/strata-engine.md` in sync when the design doc changes.
