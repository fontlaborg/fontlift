## 2025-12-02
- Updated CLI uninstall flow to try user and system scopes automatically (subject to permissions) and added a unit test to cover cross-scope fallback; removed the duplicate `fake_registry_root` definition in the macOS platform crate to restore compilation. Ran `cargo test --workspace --exclude fontlift-python` (fails in `fontlift-platform-mac` fake registry tests: environment left set after failing round-trip) and `RUST_TEST_THREADS=1 cargo test --workspace --exclude fontlift-python` (same two macOS failures; CLI/core tests pass).
- Added macOS cleanup pruning (Core Text unregister of missing registrations) plus Adobe/Microsoft cache clearing; CLI cleanup now supports `--prune-only`/`--cache-only` with dry-run messaging and verbose prune counts. Added CLI unit coverage for cleanup flag behavior.
- `cargo test --workspace` currently fails while linking `fontlift-python` against the system Python 3.13 runtime (missing libpython symbols); reran `cargo test --workspace --exclude fontlift-python` (pass).
- CLI list output now always sorts; path-only output is deduped by default and docs updated accordingly. Ran `cargo test -p fontlift-cli` (pass).
- Implemented descriptor-based macOS font listing with PostScript/family/style/format metadata and scope tagging; `FontInfo` now carries optional scope and Python bindings expose it via `.dict()`.
- Added safe Core Text trait handling to avoid panics when descriptors omit traits; new unit tests cover metadata extraction and scope detection.
- `cargo test --workspace --exclude fontlift-python` (pass). `fontlift-python` still fails to link against the system Python 3.13 runtime; rerun on Python 3.12 or with dynamic lookup to exercise those tests.
- Added PyO3 `FontInfo` class exposed to Python bindings with `.dict()` helper; list APIs now return typed objects. New unit test covers field exposure and dict serialization.
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
