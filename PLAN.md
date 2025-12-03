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
- **Python** missing: typed `FontliftFontSource/FontliftFontFaceInfo`, name-based operations, cleanup/prune toggles, JSON-friendly outputs, Fire-compatible CLI, maturin/pyproject for wheels, version sync with Cargo.
- **Verification** missing: golden-output tests, fixture fonts, CI matrix for macOS/Windows, migration docs from legacy CLIs.

## Workstreams (actionable)

### WS0 — Parity audit & build hygiene
- [x] Catalogue every command/flag/behavior in both legacy CLIs; update `FEATURE_MATRIX.md` with a parity checklist.
- [x] Fix macOS build blockers (Core Text constants, CFDictionary construction, feature gates) so `cargo test --workspace` passes locally.
- [x] Gate Windows-specific code to compile cleanly on macOS; add minimal mock stubs for non-target hosts to keep tests green.

### WS1 — macOS parity (Swift → Rust)
- Implement Core Text register/unregister with scope options; copy fonts into user/system dirs with system font protection and conflict detection/auto-resolve. *(Done 2025-12-03: installs now copy into scope-specific dirs, auto-unregister/retry on CT conflicts, replace-on-reinstall in fake/user scopes, and `is_font_installed` inspects Core Text registrations/paths.)*
- Implement listing via `CTFontManagerCopyAvailableFontURLs` + descriptor metadata (PostScript/full/family/style/format) and scope tagging. *(Done 2025-12-02: descriptor-based listing with scope tagging and trait extraction.)*
- Implement cleanup: prune missing registrations, clear ATS caches, clear Adobe/Microsoft caches; flags `--prune-only`, `--cache-only`, `--admin`. *(Done 2025-12-02: pruning + cache clearing wired with flags and dry-run support.)*
- Add simulation/dry-run and fake registry mode for tests. *(Done 2025-12-03: `FONTLIFT_FAKE_REGISTRY_ROOT` plus CLI dry-run paths.)*

### WS2 — Windows parity (C++ → Rust)
- Wire install/uninstall/remove to registry + GDI with file copy to per-user/system fonts, admin detection, conflict auto-removal.
- Implement listing from registry + fonts directory with metadata, deduplication, and scope detection.
- Implement cleanup: prune missing registry entries, clear FontCache service data, clear Adobe caches; support `--prune-only`, `--cache-only`, `--admin` and exit-code parity.
- *Progress 2025-12-03:* Registry pruning and FontCache stop/clear/start flow implemented; AdobeFnt*.lst purging added under Program Files; still pending validation on Windows hosts.
- *Progress 2025-12-03:* Install path now auto-detects conflicts (path/PostScript/family-style) and unregisters/removes duplicates before copy, while refusing to touch protected system font paths.
- *Progress 2025-12-03:* Listing now prefers name table metadata via `read-fonts` (PostScript/family/subfamily/full name) for registry + directory entries with scope tagging and deduplication.
- *Progress 2025-12-03:* Added cross-platform unit coverage for Adobe cache discovery/removal and ProgramFiles vs ProgramFiles(x86) deduplication to harden cleanup logic pending Windows host validation.
- *Progress 2025-12-03:* Registry values are stored as filenames when installed under Fonts roots and normalized back to absolute paths for listing/uninstall; cleanup now stops both FontCache and optional WPF font cache services before deleting cache files.
- *Progress 2025-12-03:* Registry unregister/prune now normalize filename-only entries (case-insensitive) to prevent false positives and stale values when removing/pruning fonts.

### WS3 — Unified CLI ergonomics
- Align commands/flags with legacy binaries: aliases, batch install/remove, name- and path-based operations, JSON output, quiet/verbose, dry-run, deterministic sorting.
- Add migration-safe help text and consistent error/exit codes; add shell completion generation.
  - [x] Shell completion generation via `fontlift completions <shell>` writing to stdout.
  - [x] Exit code alignment with legacy binaries for common failure cases.
  - [x] Implemented `list --json` output with deterministic sorting and deduplication to stabilize scripting surface.
  - [x] Added batch file/dir handling plus `--dry-run`/`--quiet`/`--verbose` toggles for install/uninstall/remove commands.

### WS4 — Python bindings & packaging
- Expose full surface: typed `FontliftFontSource`/`FontliftFontFaceInfo`, list/install/uninstall/remove/cleanup with scope/admin/prune/cache/dry-run options, name-based ops, JSON-friendly return values.
- Add Fire-based CLI entry mirroring Rust CLI; keep behavior parity.
- Ship `pyproject.toml` + `maturin` workflow for universal2 macOS and win64/aarch64 wheels; sync versioning with Cargo.
- Progress 2025-12-03: PyO3 exports `FontSource` + `FontFaceInfo` classes (scope/format/face_index metadata), list/install/uninstall/remove now route through `FontliftFontSource`; cleanup/prune/cache toggles added to Python API; name-based uninstall/remove with dry-run support and Fire CLI cleanup toggles are wired; Fire CLI now mirrors Rust JSON list rendering and quiet/verbose/dry-run messaging; remaining gap is validating wheel packaging on macOS/Windows.

- Add font fixtures (TTF/OTF/TTC) and golden-output recordings from legacy binaries for list/install/uninstall/remove/cleanup.
- *Progress 2025-12-03:* Added Atkinson Hyperlegible TTF/OTF/TTC fixtures under `tests/fixtures/fonts`; golden outputs remain.
- Add Rust integration tests per platform using temp dirs and admin-check mocks; add Python `pytest` integration via `maturin develop`. *(Done 2025-12-03: macOS fake-registry integration coverage plus pytest harness with import skips when the extension isn't built.)*
- Stand up CI matrix (macOS + Windows) running `cargo test`, CLI smoke tests with fixtures, and Python wheel build/test; enforce coverage (>80% initial). *(Done 2025-12-03: Added GitHub Actions CI for macOS 14 + Windows runners with rustfmt/clippy, platform-scoped cargo test, maturin develop, and pytest against the Python bindings.)*

### WS6 — Documentation & release
- Update README/USAGE/FEATURE_MATRIX with parity status, flags, migration guide, and Python examples. *(Updated 2025-12-03: added packaging section documenting build.sh wheel output and Windows prerequisites.)*
- Harden build script (`build.sh`) and Windows packaging and document prerequisites.
- Publish release checklist, CHANGELOG entries, and distribution plan (crates.io, GitHub releases with binaries, PyPI wheels). *(Done 2025-12-03: added RELEASE_CHECKLIST.md for Rust crates + PyPI/GitHub steps.)*

### WS7 — Out-of-process font validation pipeline (Security/Robustness)
**Goal:** Move all "dangerous" font parsing into a separate, short-lived, resource-limited helper process using `read-fonts` for structural validation before the OS font stack ever sees the bytes.

- [x] Create `fontlift-validator` binary crate (`crates/fontlift-validator`) as a small helper process:
  - Accept list of paths + optional config (max size, timeout, allowed formats) via CLI flags or JSON stdin.
  - Use `read-fonts` to structurally parse fonts and extract metadata (PostScript name, family, style, weight, italic).
  - Enforce resource limits: max file size (default 64MB), timeout watchdog, extension + MIME sniffing.
  - Return JSON array of `{ ok: bool, info?: FontliftFontFaceInfo, error?: String }`.
  - Sanitize error strings (no internal paths/stack traces).
- [x] Add `validation_ext` module to `fontlift-core`:
  - `ValidatorConfig { max_file_size_bytes, timeout_ms, allow_collections }`.
  - `validate_and_introspect(paths, config) -> FontResult<Vec<Result<FontliftFontFaceInfo, FontError>>>`.
  - Spawn validator child process, send paths/config over stdin, parse JSON response.
- [x] Wire validator into macOS install path (`MacFontManager::install_font`):
  - Early call to `validate_single` before copy/registration when `validation_config` is set on manager.
  - Abort with `FontError::InvalidFormat` if validation fails.
  - Manager exposes `with_validation(config)` constructor and `set_validation_config()` setter.
- [x] Wire validator into Windows install path (`WinFontManager::install_font`) when fleshing out Windows parity.
- [x] Expose validation config in Python bindings:
  - Added `strict: bool = False` parameter on `install()` function and `FontliftManager.install_font()` method.
  - When `strict=True`, creates manager with `ValidatorConfig::default()` for out-of-process validation.
- [x] Expose validation in CLI:
  - `--no-validate` flag (default: validate on install).
  - `--validation-strictness {lenient,normal,paranoid}` presets.
  - Batch validation for installs (amortize process overhead).
- [x] Add unit tests:
  - Known-bad font samples (random binary with `.ttf` extension) → clean error, no crash.
  - Max size / timeout behaviour with dummy files.
- [x] Add integration tests:
  - Malformed font fixture + `fontlift install` → fails with `InvalidFormat`, no CT registration.
  - Tests in `crates/fontlift-cli/tests/macos_fake_registry_tests.rs` verify CLI and manager-level validation.

### WS8 — Transactional operation journal for crash-safe install/remove
**Goal:** Wrap multi-step operations (copy, register, unregister, delete, clear cache) in a small operation journal so interrupted operations can be auto-repaired on next run.

- [x] Add `journal` module to `fontlift-core`:
  - `JournalAction` enum: `CopyFile { from, to }`, `RegisterFont { path, scope }`, `UnregisterFont { path, scope }`, `DeleteFile { path }`, `ClearCache { scope }`.
  - `JournalEntry { id: Uuid, started_at, completed, actions: Vec<JournalAction>, current_step: usize }`.
  - Journal file location: macOS `~/Library/Application Support/FontLift/journal.json`, Windows `%LOCALAPPDATA%\FontLift\journal.json`.
  - Atomic write pattern (write to `.tmp` then rename).
- [x] Implement journal helpers:
  - `journal_path() -> PathBuf` (platform-specific).
  - `load_journal() -> Vec<JournalEntry>`, `save_journal(&[JournalEntry])`.
  - `record_operation(actions) -> JournalEntry`, `mark_step(entry_id, step)`, `mark_completed(entry_id)`.
- [x] Implement crash recovery logic:
  - `recover_incomplete_operations<F>(manager, handler)` iterates incomplete entries and rolls forward or back.
  - Policy per action: CopyFile (check exists/size), RegisterFont (check is_font_installed), UnregisterFont (retry if still installed), DeleteFile (delete if exists).
- [x] Wire journal into `MacFontManager::install_font`:
  - Build actions list: CopyFile (if needed), RegisterFont.
  - Call `record_operation`, execute actions with `mark_step` after each, then `mark_completed`.
- [x] Wire journal into `MacFontManager::remove_font` (UnregisterFont → DeleteFile).
- [x] Wire journal into Windows manager when fully implemented.
- [x] Add CLI `fontlift doctor` command:
  - Runs `recover_incomplete_operations` and prints actions taken.
  - Optionally auto-run at process start (behind flag or env var).
- [x] Add unit tests:
  - Simulate starting entry, advancing steps, calling recovery → verify expected rollback/forward.
- [x] Add integration tests:
  - `mac_fake_registry_doctor_recovers_incomplete_copy`: simulates crash after CopyFile record, verifies doctor recovery.
  - `mac_fake_registry_doctor_recovers_incomplete_delete`: simulates crash after DeleteFile record, verifies recovery.

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

*Last updated: 2025-12-03*
