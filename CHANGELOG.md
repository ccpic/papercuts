# Changelog

## [0.4.0] - 2026-08-14

### Added

- New terminal resolution reason `won't-fix` for records confirmed real but deliberately not repaired (one-off/environment-specific facts, cost exceeding benefit, or unchangeable external root cause).

## [0.3.0] - 2026-08-14

### Added

- Filter `list` by repeatable full canonical IDs and request a compact JSON triage projection.
- Move cuts between `open` and `blocked` with atomic, idempotent `block` and `unblock` commands.
- Record terminal resolution reasons: `fixed`, `already-fixed`, `duplicate`, `false-observation`, or `obsolete`.
- Return explicit changed-now, already-in-state, and dry-run ID sets from lifecycle mutations.

### Changed

- Advance the machine contract to version 2 for lifecycle states, events, and mutation output shapes.
- Treat resolved state as terminal while allowing blocked cuts to resolve directly.

### Fixed

- Restore the original log length after a partial append failure on Windows by reopening the locked path with ordinary write access.

## [0.2.0] - 2026-07-16

### Added

- Attach bounded evidence to `add` with `--cmd`, `--exit`, `--stderr-file`, and `--evidence`.
- Resolve multiple IDs or unique prefixes atomically in one `resolve` command.

### Changed

- Redact common credential assignments, authorization values, high-entropy tokens, and URL userinfo before evidence is returned or stored. Redaction remains best-effort.
- Reject non-regular, non-UTF-8, and larger-than-1-MiB stderr inputs; truncate sanitized stored stderr to 4096 UTF-8 bytes.
- Preserve the single-ID resolve response while returning sorted, deduplicated records for multi-ID resolves and warnings for already-resolved IDs.
- Expand `schema` with the evidence record and multi-ID resolve contracts.
- Expand doctor/fold test coverage for malformed records, duplicate cuts, ID conflicts, orphan resolves, conflict markers, torn lines, and append recovery.
- Limit the published crate to source, tests, and release documentation.

### Fixed

- Roll back failed batch appends so partial multi-resolve writes do not corrupt the append-only log.
- Accept leading-hyphen values for evidence and resolution notes without swallowing later options.
- Preserve paths and URLs during best-effort credential redaction while covering token-only URL userinfo and lowercase compound credential keys.

## [0.1.0] - 2026-07-10

- Initial release.

[0.3.0]: https://github.com/treygoff24/papercuts/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/treygoff24/papercuts/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/treygoff24/papercuts/releases/tag/v0.1.0
