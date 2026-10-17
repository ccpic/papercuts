# papercuts

A tiny CLI that gives AI agents a complaint box.

Agents hit friction constantly — dead-end tool calls, broken links, missing helpers, footgun configs — and silently push through without telling anyone. The signal evaporates. `papercuts` gives an agent a one-line way to file the complaint at the moment it happens, and gives you (or another agent) a way to review the backlog and fix the actual problems in your repo, your tooling, your docs.

```
$ papercuts add "yarn web:test with a root-relative path finds no files; the workspace test cwd is apps/web" --tag tooling
{"ok":true,"data":{"changed":true,"record":{"kind":"cut","id":"pc_9f2c41d0a8b3","ts":"2026-07-09T21:14:03.412Z","agent":"claude-code","text":"yarn web:test with a root-relative path finds no files; the workspace test cwd is apps/web","tags":["tooling"],"severity":"minor",...}},"meta":{"contract":3,"file":"/repo/.papercuts.jsonl","agent_source":"detected"}}
```

The idea comes from [a tool Steve Ruiz built](https://x.com/steveruizok) for his own repos: once agents had a place to complain, they immediately surfaced real workflow defects — quoting bugs, wrong test working directories, YAML footguns — that they'd been eating silently for months.

## Install

```bash
cargo install papercuts
```

## How it works

Papercuts live in an **append-only JSONL file** — by default `.papercuts.jsonl` at your repo root, so every complaint shows up in `git diff` and travels with the repo. No server, no sync, no telemetry. The file is the product.

```bash
papercuts add "text"                     # file a papercut (also: papercuts log, or pipe stdin to add -)
papercuts list                           # open papercuts, severity-first then newest, JSON envelope
papercuts list --projection triage       # compact backlog view for ranking
papercuts list --id pc_9f2c41d0a8b3     # fetch exact records; --id is repeatable
papercuts list --format md               # human review digest
papercuts block pc_9f2c --note "waiting on upstream" # remove a genuine blocker from the open queue
papercuts unblock pc_9f2c                # return a blocked cut to the open queue
papercuts resolve pc_9f2c --reason fixed # append a terminal resolution (unique ID prefix ok)
papercuts resolve pc_9f2c pc_a81e        # resolve several atomically
papercuts add "tool failed" --cmd 'tool --flag' --exit 1 --stderr-file /tmp/stderr
papercuts add "bad response" --evidence 'request_id=abc123'
papercuts schema                         # full machine contract — agents self-orient with this
papercuts doctor                         # validate the log file
```

- **Agent-first contract**: stdout is data only; one JSON envelope per command; structured errors on stderr with stable codes, documented exit codes, and a paste-ready `suggested_fix`. `papercuts schema` returns the whole contract.

### Repository debt ledgers

A second record kind carries *repository debt* — the standing baseline tables a repo ratchets
down (type-checker baselines, exemption tables, coverage floors). One ledger is one record,
addressed by its repository path, so a migration re-run at a new wall-clock time is a no-op
rather than a stack of duplicates.

```
papercuts add --kind debt --owner platform-maintainer \
  --baseline-count 714 --path scripts/typecheck/pyright-baseline.json
papercuts list --kind debt --projection triage     # debt is invisible to a bare `list`
papercuts rebaseline pc_83fd5cb64d61 --baseline-count 735 --note "baseline grew back after new violations"
```

`owner`, `baseline_count`, and `path` are required on a debt and refused on a cut. The crate
*stores* `baseline_count` without comparing it: the ratchet verdict belongs to the repository's
own checker, which reads `list --kind debt` and fails on growth. A resolved debt can be reopened
with `rebaseline` — debt recurs, and `resolve` is otherwise terminal.
- **Concurrency-safe**: multiple agents on one file are fine (advisory locking, atomic appends, self-healing torn lines).
- **Deterministic**: content-addressed IDs, stable sort, reproducible-clock override for tests.
- **Never rewrites history**: `block`, `unblock`, and terminal `resolve` operations append events; the log is a journal, not a database.
- **Explicit lifecycle results**: mutation envelopes distinguish IDs changed now, already in target state, and dry-run candidates. Resolution reasons are `fixed`, `already-fixed`, `duplicate`, `false-observation`, `obsolete`, or `won't-fix` (deliberately not repaired — one-off facts, cost exceeding benefit, or unchangeable external root cause; requires a note).
- **Evidence is bounded and redacted**: `add` can attach a failed command (`--cmd`), exit status (`--exit`), UTF-8 stderr file (`--stderr-file`), or free-form note (`--evidence`). `--stderr-file` rejects non-regular files and inputs over 1 MiB before sanitized stderr is stored up to 4096 UTF-8 bytes. Redaction is best-effort, so never feed raw environment dumps.

## Give your agents the pen

Paste this into your `CLAUDE.md` / `AGENTS.md` / system prompt:

```markdown
## Papercuts

When you hit friction during work — a dead-end tool call, a broken link, a
misleading doc, a footgun config, a missing helper — file it before moving on:

    papercuts add "<what you hit and what would have prevented it>" --tag <area>

Don't stop working; file it and push through. Severity: minor (default) for
annoyances, major for time sinks, blocker for hard walls. Keep the returned ID:
after a durable fix passes the original failing path, resolve it with a reason.
Use `block` only for a confirmed external dependency, not as a substitute for
repair. Run `papercuts schema` once if you need the full contract. Attach
`--cmd`, `--exit`, or `--stderr-file` when filing tool failures; never feed raw
environment dumps.
```

Then periodically: `papercuts list --format md` and fix what your agents keep tripping over.

## Team modes

**Committed (default).** `.papercuts.jsonl` is a normal tracked file — papercuts appear in diffs and PRs. Add this to `.gitattributes` so parallel branches merge cleanly:

```
.papercuts.jsonl merge=union
```

Duplicate lines after a merge are harmless — the fold is first-wins and `papercuts add` is duplicate-safe.

**Private.** Prefer not to commit them? `echo .papercuts.jsonl >> .gitignore`, or point `PAPERCUTS_FILE` somewhere else entirely. Outside a git repo, papercuts go to `~/.papercuts/log.jsonl`.

## Contract

Everything an agent needs is in `papercuts schema`: contract version 3, commands and flags with read-only/appends annotations, env vars (`PAPERCUTS_FILE`, `PAPERCUTS_AGENT`, `PAPERCUTS_NOW`), record shapes, error codes, and the exit-code dictionary (0 success · 2 usage · 65 bad input · 66 not found · 70 internal · 74 I/O · 75 lock timeout, retryable · 77 permission denied · 78 config). Empty results are exit 0, never errors.

## License

MIT
