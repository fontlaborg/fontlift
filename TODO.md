# fontlift TODO (2025-12-01)

- [x] Audit legacy `fontlift-mac-cli` and `fontlift-win-cli` commands/flags/behaviors and update `FEATURE_MATRIX.md` with a parity checklist.
- [x] Fix macOS build blockers (Core Text constants, CFDictionary construction) so `cargo test --workspace` passes on macOS; gate Windows code for non-Windows hosts.
- [x] Implement macOS install/uninstall/remove using Core Text register/unregister with scope options, safe copy to user/system dirs, conflict detection/auto-resolve, and system font protection.
- [x] Implement macOS listing via `CTFontManagerCopyAvailableFontURLs` with descriptor-based metadata and scope tagging.
- [x] Implement macOS cleanup (prune missing registrations + ATS/Adobe/Microsoft cache clearing) with `--prune-only`, `--cache-only`, `--admin` flags.
- [x] Add macOS simulation/dry-run + fake registry mode to mirror Swift testing features (`FONTLIFT_FAKE_REGISTRY_ROOT`, CLI `--dry-run`).
- [~] Implement Windows install/uninstall/remove using registry + GDI, file copy to per-user/system fonts, admin detection, and conflict auto-removal.
  - Conflict detection now auto-uninstalls conflicting registry/file entries before copying while protecting system font paths.
- [x] Implement Windows listing from registry + fonts directory with metadata extraction, deduplication, and scope detection.
- [~] Implement Windows cleanup (registry prune, FontCache service reset, Adobe cache clearing) with `--prune-only`, `--cache-only`, `--admin` and exit-code parity.
  - Registry pruning and FontCache service stop/clear/start paths implemented; AdobeFnt*.lst purge added under Program Files; needs validation on a Windows host.
  - Added cross-platform unit coverage for Adobe cache discovery/removal and ProgramFiles/ProgramFiles(x86) deduplication to reduce cleanup regressions.
- [x] Add cross-platform conflict detection in `fontlift-core`; duplicate handling and system-font protection helpers added.
  - Conflict detection helper added and wired into Windows installs to auto-remove duplicate registrations/files before copying.
- [x] Expand CLI to match legacy ergonomics: aliases, batch file/dir installs, name- and path-based uninstall/remove, `-p/-n/-s`, `--json`, `--dry-run`, `--quiet/--verbose`, deterministic sorting, and help text updates.
- [x] Add CLI `list` JSON output with deterministic sorting and deduplication for repeat entries.
- [x] Add shell completion generation via `fontlift completions <shell>` and align exit codes with legacy binaries.
- [x] Expand Python bindings: typed `FontliftFontSource`/`FontliftFontFaceInfo` exposed to Python with `.dict()` for JSON; cleanup/prune/cache toggles added; name-based operations and Fire CLI parity completed.
- Fire CLI now mirrors Rust `list` JSON/flags, quiet/verbose/dry-run messaging, and default deduped path output; functional `list_fonts` now returns dictionaries matching documented examples.
- [x] Add `pyproject.toml` + packaging workflow for platform wheels; sync Python versioning with git tags via hatch-vcs.
- [x] Create font fixtures (TTF/OTF/TTC) and golden-output recordings from legacy binaries for list/install/uninstall/remove/cleanup.
  - Added Atkinson Hyperlegible TTF/OTF/TTC fixtures under `tests/fixtures/fonts`.
  - Created `tests/fixtures/golden_outputs/list_json_schema.json` documenting expected JSON structure.
  - Integration tests validate deterministic JSON output and schema compliance.
- [x] Add Rust integration tests per platform with temp dirs and admin-check mocks; add Python `pytest` integration via `maturin develop`.
- [x] Stand up CI matrix (macOS + Windows) running `cargo test`, CLI smoke tests with fixtures, and Python wheel build/test with coverage gates.
- [x] Update README/USAGE/FEATURE_MATRIX with parity status, migration guide, and Python examples; refresh build script (`build.sh`) and Windows packaging docs.
  - Status docs refreshed with macOS/Windows/Python parity summary and cleanup toggles; FEATURE_MATRIX updated 2025-12-03; packaging section added covering build.sh/wheel outputs and Windows prerequisites.
- [x] Publish release checklist, CHANGELOG entries, and keep WORK log updated alongside TODO status.

## WS7 — Out-of-process font validation pipeline

- [x] Create `fontlift-validator` binary crate with `read-fonts` parsing, max-size/timeout enforcement, and JSON output.
- [x] Add `validation_ext` module to `fontlift-core` with `ValidatorConfig` and `validate_and_introspect` API.
- [x] Wire validator into macOS `MacFontManager::install_font` for pre-flight validation. *(Manager now validates when `validation_config` is set; CLI also pre-validates.)*
- [ ] Wire validator into Windows `WinFontManager::install_font` when Windows parity is complete.
- [x] Expose `strict`/`validation_config` in Python bindings (`install`, `cleanup`). *(Added `strict=False` parameter to `install()` and `FontliftManager.install_font()`.)*
- [x] Add CLI `--no-validate` and `--validation-strictness {lenient,normal,paranoid}` flags.
- [x] Add unit tests for malformed fonts, max-size, and timeout behaviour.
- [x] Add integration tests: malformed font fixture → `InvalidFormat` error, no OS registration.
  - Tests in `crates/fontlift-cli/tests/macos_fake_registry_tests.rs` verify CLI and manager-level validation.

## WS8 — Transactional operation journal

- [x] Add `journal` module to `fontlift-core` with `JournalAction`, `JournalEntry`, and atomic file writes.
- [x] Implement journal helpers: `journal_path`, `load_journal`, `save_journal`, `record_operation`, `mark_step`, `mark_completed`.
- [x] Implement `recover_incomplete_operations` with roll-forward/rollback policy per action type.
- [x] Wire journal into `MacFontManager::install_font` (CopyFile, RegisterFont actions).
- [x] Wire journal into `MacFontManager::remove_font` (UnregisterFont, DeleteFile actions).
- [ ] Wire journal into Windows manager when fully implemented.
- [x] Add CLI `fontlift doctor` command for manual recovery.
- [x] Add unit tests for journal entry lifecycle and recovery logic.
- [x] Add integration tests with fault injection (panic mid-install) and recovery verification.
  - `mac_fake_registry_doctor_recovers_incomplete_copy`: simulates crash after CopyFile journal record, verifies doctor recovery.
  - `mac_fake_registry_doctor_recovers_incomplete_delete`: simulates crash after DeleteFile journal record, verifies doctor recovery.
