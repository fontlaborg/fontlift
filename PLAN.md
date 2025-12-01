# FontLift Superset Convergence Plan (2025-12-01)

**One-sentence scope:** Deliver `fontlift` (Rust library + CLI + Python bindings) as a strict superset of `fontlift-mac-cli` and `fontlift-win-cli`, matching every behavior while adding a unified, testable, and convenient API surface.

## Current State (facts)
- Workspace crates exist (`core`, `platform-mac`, `platform-win`, `cli`, `python`), but macOS Core Text calls fail to compile; Windows path is unverified on non-Windows.
- CLI surface is minimal (list/install/uninstall/remove/cleanup) and lacks legacy flags (`--prune-only`, `--cache-only`, `--json`, batch ops, simulation/dry-run, aliases).
- Python bindings expose only path-based operations; no metadata, name-based operations, or cleanup/prune options; no packaging story for PyPI wheels.
- No automated parity checks against Swift/C++ CLIs; no fixture fonts; tests currently fail on macOS.

## Gap analysis vs legacy CLIs
- **macOS (Swift CLI)** missing: Core Text register/unregister, conflict detection/auto-resolve, rich metadata extraction, prune + third-party cache cleanup, system font protections with overrides, simulation/fake registry mode, environment overrides, complete flag set.
- **Windows (C++ CLI)** missing: real registry + GDI wiring for install/uninstall/list, conflict removal, cache cleanup (FontCache service + Adobe caches), admin detection paths, robust registry pruning, exit-code parity.
- **Cross-platform/CLI UX** missing: aliases (`i/u/rm/c/l`), `-p/-n/-s` toggles, JSON output, batch file/dir handling, quiet/verbose, dry-run, deterministic sorting, consistent error strings.
- **Python** missing: typed `FontInfo`, name-based operations, cleanup/prune toggles, JSON-friendly outputs, Fire-compatible CLI, maturin/pyproject for wheels, version sync with Cargo.
- **Verification** missing: golden-output tests, fixture fonts, CI matrix for macOS/Windows, migration docs from legacy CLIs.

## Workstreams (actionable)

### WS0 — Parity audit & build hygiene
- [x] Catalogue every command/flag/behavior in both legacy CLIs; update `FEATURE_MATRIX.md` with a parity checklist.
- [x] Fix macOS build blockers (Core Text constants, CFDictionary construction, feature gates) so `cargo test --workspace` passes locally.
- [x] Gate Windows-specific code to compile cleanly on macOS; add minimal mock stubs for non-target hosts to keep tests green.

### WS1 — macOS parity (Swift → Rust)
- Implement Core Text register/unregister with scope options; copy fonts into user/system dirs with system font protection and conflict detection/auto-resolve.
- Implement listing via `CTFontManagerCopyAvailableFontURLs` + descriptor metadata (PostScript/full/family/style/format) and scope tagging.
- Implement cleanup: prune missing registrations, clear ATS caches, clear Adobe/Microsoft caches; flags `--prune-only`, `--cache-only`, `--admin`.
- Add simulation/dry-run and fake registry mode for tests.

### WS2 — Windows parity (C++ → Rust)
- Wire install/uninstall/remove to registry + GDI with file copy to per-user/system fonts, admin detection, conflict auto-removal.
- Implement listing from registry + fonts directory with metadata, deduplication, and scope detection.
- Implement cleanup: prune missing registry entries, clear FontCache service data, clear Adobe caches; support `--prune-only`, `--cache-only`, `--admin` and exit-code parity.

### WS3 — Unified CLI ergonomics
- Align commands/flags with legacy binaries: aliases, batch install/remove, name- and path-based operations, JSON output, quiet/verbose, dry-run, deterministic sorting.
- Add migration-safe help text and consistent error/exit codes; add shell completion generation.
  - [x] Shell completion generation via `fontlift completions <shell>` writing to stdout.
  - [x] Exit code alignment with legacy binaries for common failure cases.
  - [x] Implemented `list --json` output with deterministic sorting and deduplication to stabilize scripting surface.
  - [x] Added batch file/dir handling plus `--dry-run`/`--quiet`/`--verbose` toggles for install/uninstall/remove commands.

### WS4 — Python bindings & packaging
- Expose full surface: typed `FontInfo`, list/install/uninstall/remove/cleanup with scope/admin/prune/cache/dry-run options, name-based ops, JSON-friendly return values.
- Add Fire-based CLI entry mirroring Rust CLI; keep behavior parity.
- Ship `pyproject.toml` + `maturin` workflow for universal2 macOS and win64/aarch64 wheels; sync versioning with Cargo.

### WS5 — Tests, fixtures, and parity verification
- Add font fixtures (TTF/OTF/TTC) and golden-output recordings from legacy binaries for list/install/uninstall/remove/cleanup.
- Add Rust integration tests per platform using temp dirs and admin-check mocks; add Python `pytest` integration via `maturin develop`.
- Stand up CI matrix (macOS + Windows) running `cargo test`, CLI smoke tests with fixtures, and Python wheel build/test; enforce coverage (>80% initial).

### WS6 — Documentation & release
- Update README/USAGE/FEATURE_MATRIX with parity status, flags, migration guide, and Python examples.
- Harden build script (`build.sh`) and Windows packaging and document prerequisites.
- Publish release checklist, CHANGELOG entries, and distribution plan (crates.io, GitHub releases with binaries, PyPI wheels).

## Milestones
- **M1:** Build passes on macOS and parity checklist completed (WS0).
- **M2:** macOS parity validated against Swift CLI fixtures (WS1).
- **M3:** Windows parity validated against C++ CLI fixtures (WS2).
- **M4:** Unified CLI + Python parity done (WS3–WS4).
- **M5:** CI matrix green with parity tests and docs/release updates (WS5–WS6).

## Verification approach (superset guarantee)
- Keep a living parity checklist derived from legacy help/output; every item mapped to Rust/Python behavior or explicitly deprecated with rationale.
- Golden-output tests compare Rust CLI/Python binding results to recorded legacy outputs for core commands on each platform.
- Contract tests ensure Python JSON structures match Rust CLI JSON output.
- Treat any missing feature or divergent behavior as a release blocker until resolved or documented.

## Architecture snapshot (for reference)
- Workspace crates: `fontlift-core` (errors, validation, traits), `fontlift-platform-mac`, `fontlift-platform-win`, `fontlift-cli`, `fontlift-python`.
- Platform APIs: macOS via Core Text/Objective-C; Windows via `windows` crate (GDI + Registry).
- Tooling: Rust 1.75+, PyO3 + maturin for bindings; clap for CLI; tests via `cargo test` + `pytest`.

*Last updated: 2025-12-01*
