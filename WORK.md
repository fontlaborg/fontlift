## 2025-12-03

### Test Suite Status

**13 macOS integration tests + 16 CLI/unit tests pass.**
**15 Windows platform unit tests pass (non-Windows host).**
**5 Python tests pass.**
**Clippy clean, fmt clean.**

---

### Latest Changes

- Made fontlift-python opt-in behind a `python-bindings` feature so workspace builds no longer require a host Python install; pyproject/build.sh/release checklist now enable the feature for wheel builds.
- Swapped the pyproject build backend to `maturin` with explicit manifest/python-source settings to silence pep517 warnings; rebuilt via `maturin develop -m crates/fontlift-python/Cargo.toml` and reran `pytest python/tests` (5 passing).
- Added `help_text_includes_all_commands` test verifying all 7 commands appear in CLI help
- Added `shell_completions_generate_for_all_shells` test for Bash/Zsh/Fish/PowerShell/Elvish
- Added `fontlift doctor` and validation docs to USAGE.md
- Added `validation_strictness_presets_parse` test for lenient/normal/paranoid CLI flags
- Added `no_validate_flag_parses` test for `--no-validate` flag
- Fixed CI workflow: changed Python build from `maturin develop` to `uv pip install -e .`
- Hardened Windows registry handling: `unregister_font_from_registry` now matches filename-only entries case-insensitively and prune normalizes registry values, preventing stale entries when fonts live under Fonts roots.
- Enabled `registry_value_matches_path` for non-Windows builds so `fontlift-platform-win` unit tests compile off-Windows; reran `cargo test -p fontlift-platform-win -- --nocapture` (15 passing).

### Windows cleanup (user-scope cache clearing)

- `fontlift cleanup` now treats user-scope cache clear `PermissionDenied` as a warning so the default command succeeds without `--admin`; system-scope still fails without elevation.
- Added CLI unit test `cleanup_skips_cache_clear_permission_denied_on_windows_user_scope` with a permissive manager stub to lock the behavior.

### Windows validation + journal wiring

- Added optional validation config to `WinFontManager` (mirroring macOS) and enabled strict installs for Python `strict=True`; Windows install/remove now journal Copy/Register and Unregister/Delete for doctor recovery.
- Tests: `cargo test --workspace` (fails linking `fontlift-python` to system Python libs on this host); `cargo test -p fontlift-platform-win -- --nocapture` passes (15 tests).

### Windows registry normalization + cache service coverage

- Registry values now store filenames when installing into Fonts roots, and registry entries normalize relative paths back to absolute scope paths for listing/uninstall/resolve.
- Uninstall/remove will now resolve renamed registry entries by matching filenames across scopes.
- Cache cleanup stops/starts both `FontCache` and optional `FontCache3.0.0.0` services before deleting cache files; added unit coverage for registry path normalization.

#### Tests
- cargo test --workspace
- cargo fmt
- cargo test -p fontlift-platform-win -- --nocapture
- cargo test -p fontlift-cli -- --nocapture
