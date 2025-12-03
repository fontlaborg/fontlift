## 2025-12-03
- Fixed build script Python binding step to point maturin at the Python crate manifest instead of the workspace root so `maturin develop` no longer fails with "missing field package".
- `maturin develop -m crates/fontlift-python/Cargo.toml --features extension-module` (pass; PyO3 deprecation warnings and missing `PyInit_fontlift_python` symbol warning unchanged).
- Added cross-platform conflict detection helper (path/PostScript/family+style) with unit coverage and wired Windows installs to auto-unregister/remove conflicting registry/file entries while protecting system font paths; user-scope copies now replace existing files safely.
- Tests: `cargo test --workspace --exclude fontlift-python`.
- Hardened macOS cache cleanup test by serializing env mutations and clearing fake-registry/test-cache env vars via guard; `clear_font_caches_removes_vendor_caches_under_override_root` now passes reliably.
- Added Windows cleanup Adobe cache purge (AdobeFnt*.lst) by scanning common Program Files TypeSupport/TypeSpt/PDFL roots; introduced helper test to pin expected search paths.
- `cargo test --workspace --exclude fontlift-python` (pass).
- Windows: added registry-based prune_missing_fonts plus FontCache service stop/clear/start in `WinFontManager`; user-scope cleanup now errors without `--admin`. `list_installed_fonts` now dedupes registry + font directories with scope tagging and `is_font_installed` resolves installed targets. Ran `cargo test --workspace --exclude fontlift-python` (pass).
- macOS install/uninstall/remove now register the copied scope path, auto-unregister/retry on Core Text conflict codes, replace fake/user installs when reinstalling, and `is_font_installed` inspects Core Text URLs plus on-disk targets; added env mutexed fake-registry tests for reinstall/is-installed coverage. `cargo test --workspace --exclude fontlift-python` (pass).
- Added Python name-based uninstall/remove with dry-run plus shared scope resolution; Fire CLI now exposes cleanup prune/cache/dry-run toggles and name-aware uninstall/remove; Python wrappers forward cleanup flags.
- `cargo check -p fontlift-python` (pass; PyO3 deprecation warnings remain); `cargo test --workspace --exclude fontlift-python` (pass). Python crate tests still skipped on this host because of system libpython 3.13 linking limitations.
- Added Python cleanup parity: `cleanup` now accepts `prune/cache/dry_run/admin` across the manager and module functions, backed by shared helper + unit tests with a fake manager.
- Verified macOS fake registry/dry-run item and reflected status in PLAN/TODO.

## 2025-12-02
- Updated CLI uninstall flow to try user and system scopes automatically (subject to permissions) and added a unit test to cover cross-scope fallback; removed the duplicate `fake_registry_root` definition in the macOS platform crate to restore compilation. Ran `cargo test --workspace --exclude fontlift-python` (fails in `fontlift-platform-mac` fake registry tests: environment left set after failing round-trip) and `RUST_TEST_THREADS=1 cargo test --workspace --exclude fontlift-python` (same two macOS failures; CLI/core tests pass).
- Added macOS cleanup pruning (Core Text unregister of missing registrations) plus Adobe/Microsoft cache clearing; CLI cleanup now supports `--prune-only`/`--cache-only` with dry-run messaging and verbose prune counts. Added CLI unit coverage for cleanup flag behavior.
- `cargo test --workspace` currently fails while linking `fontlift-python` against the system Python 3.13 runtime (missing libpython symbols); reran `cargo test --workspace --exclude fontlift-python` (pass).
- CLI list output now always sorts; path-only output is deduped by default and docs updated accordingly. Ran `cargo test -p fontlift-cli` (pass).
- Implemented descriptor-based macOS font listing with PostScript/family/style/format metadata and scope tagging; `FontliftFontFaceInfo` now carries optional scope and Python bindings expose it via `.dict()`.
- Added safe Core Text trait handling to avoid panics when descriptors omit traits; new unit tests cover metadata extraction and scope detection.
- `cargo test --workspace --exclude fontlift-python` (pass). `fontlift-python` still fails to link against the system Python 3.13 runtime; rerun on Python 3.12 or with dynamic lookup to exercise those tests.
- Added PyO3 `FontFaceInfo` class exposed to Python bindings with `.dict()` helper; list APIs now return typed objects. New unit test covers field exposure and dict serialization.
- `cargo test --workspace --exclude fontlift-python` (pass). `fontlift-python` unit tests currently fail to link against the system Python 3.13 runtime when built with `pyo3/extension-module`; needs either Python 3.12 or running tests inside Python (dynamic lookup) to resolve symbols.
- Fixed Python wheel build dependency by switching to the published `hatchling-pyo3-plugin` package name in `pyproject.toml` and `publish.sh`.
- Ran `./build.sh` (debug); all crates built and wheel produced successfully. Note: CLI `--version` check inside the script still warns, unchanged behavior from prior runs.

## 2025-12-01
- Added `fontlift completions <shell>` subcommand powered by `clap_complete`, emitting scripts to stdout for bash/zsh/fish/powershell/elvish.
- New test `completions_include_core_commands` to ensure generated scripts mention core commands.
- Ran `cargo test --workspace` (pass).
- Added CLI aliases (`l/i/u/rm/c`) and mapped clap parse errors to legacy exit codes (0 for help/version, 1 otherwise).
- Added tests for alias coverage and exit-code mapping; `cargo test --workspace` passes.
- Removed `build-macos.sh`; unified on `build.sh` to eliminate shell completion ambiguity (`./build` now resolves uniquely).
- Updated PLAN/TODO references to reflect single build script.
- Added core `protection` helpers for system font detection and deterministic deduplication; wired CLI list rendering to use shared dedupe logic and reused protection checks in macOS manager.
- New core tests for protection/dedup helpers; all crates pass `cargo test --workspace`.
- Extended CLI ergonomics: batch file/dir install/uninstall/remove support, global `--dry-run/--quiet/--verbose` flags, and deterministic input collection.
- Added CLI tests for directory expansion and dry-run no-op behavior; ran `cargo fmt` and `cargo test --workspace` (pass).
- Updated README/USAGE to document multi-file installs and dry-run/quiet/verbose flags; checked off TODO/PLAN items for CLI ergonomics.
- Added PyO3 build.rs to inject macOS dynamic lookup flags; `cargo test --workspace` now succeeds building `fontlift-python`.
- Swapped Python module name to `fontlift._native`, added hatch-vcs-based pyproject with Fire CLI `fontliftpy`, and added Python package scaffolding.
- Introduced release GitHub Action for tag-based releases (wheels + crates) and manual `publish.sh` helper; added hatch-friendly build steps to `build.sh`.
